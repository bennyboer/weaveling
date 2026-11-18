use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::Agent;
use inbox::{InMemoryInbox, Inboxed};
use messaging::{FakePublisher, Listener, Message};
use outbox::InMemoryOutbox;
use outline_contract::{AttachmentDTO, SECTION_ADDED, SectionStateDTO};
use outline_core::{Attachment, OutlineId, SectionId, SectionTitle};
use outline_messaging::TellStateListener;
use serde_json::json;
use transacting::InMemoryTransactions;

use crate::wiring;
use crate::wiring::Wired;

async fn a_workbench() -> Wired {
    wiring::a_workbench_where_the_author_owns(Arc::new(FixedClock::new(clock::at(1_000)))).await
}

async fn an_outline(wired: &Wired) -> OutlineId {
    wired
        .outlines
        .open("project_1", &eventsourcing::an_author())
        .await
        .expect("opening should succeed")
        .id
}

async fn a_section(
    wired: &Wired,
    outline: &OutlineId,
    under: Option<SectionId>,
    after: Option<SectionId>,
    titled_as: &str,
) -> SectionId {
    wired
        .outlines
        .add(
            &outline.to_string(),
            under,
            after,
            SectionTitle::new(titled_as).expect("the title should be usable"),
            None,
            &eventsourcing::an_author(),
        )
        .await
        .expect("adding a section should succeed")
        .section
}

async fn attached(wired: &Wired, outline: &OutlineId, scene: &str, to: SectionId) {
    wired
        .outlines
        .attach(
            &outline.to_string(),
            Attachment::scene(scene),
            to,
            None,
            None,
            &eventsourcing::an_author(),
        )
        .await
        .expect("attaching should succeed");
}

async fn version_of(wired: &Wired, outline: &OutlineId) -> u64 {
    wired
        .outlines
        .standing(outline)
        .await
        .expect("the outline should stand")
        .version
        .count()
}

#[tokio::test]
async fn a_change_is_told_as_the_whole_outline_in_reading_order() {
    // given: an outline
    let wired = a_workbench().await;
    let outline = an_outline(&wired).await;

    // when: two chapters are added, a part under the first, and a scene in the part
    let first = a_section(&wired, &outline, None, None, "Morning").await;
    let second = a_section(&wired, &outline, None, Some(first), "Market").await;
    let part = a_section(&wired, &outline, Some(first), None, "Harbour").await;
    attached(&wired, &outline, "scene_harbour", part).await;
    wired.settle().await;

    // then: the last state told is the whole outline, each part after its parent
    let told = wired.states.last();
    assert_eq!(told.outline, outline.to_string());
    assert_eq!(told.project, "project_1");
    assert_eq!(told.version, version_of(&wired, &outline).await);
    assert!(!told.discarded);
    assert_eq!(
        told.sections,
        vec![
            SectionStateDTO {
                section: first.to_string(),
                parent: None,
                title: "Morning".to_owned(),
                attachments: vec![],
            },
            SectionStateDTO {
                section: part.to_string(),
                parent: Some(first.to_string()),
                title: "Harbour".to_owned(),
                attachments: vec![AttachmentDTO::Scene {
                    id: "scene_harbour".to_owned(),
                }],
            },
            SectionStateDTO {
                section: second.to_string(),
                parent: None,
                title: "Market".to_owned(),
                attachments: vec![],
            },
        ]
    );
}

#[tokio::test]
async fn each_version_is_told_once_and_each_newer_than_the_one_before() {
    // given: an outline
    let wired = a_workbench().await;
    let outline = an_outline(&wired).await;

    // when: it is changed a few times
    let first = a_section(&wired, &outline, None, None, "Morning").await;
    a_section(&wired, &outline, None, Some(first), "Market").await;
    attached(&wired, &outline, "scene_harbour", first).await;
    wired.settle().await;

    // then: the versions told rise one after the other, up to the outline's own
    let versions: Vec<u64> = wired
        .states
        .told()
        .into_iter()
        .map(|told| told.version)
        .collect();
    assert!(
        versions.windows(2).all(|pair| pair[0] < pair[1]),
        "{versions:?}"
    );
    assert_eq!(
        versions.last().copied(),
        Some(version_of(&wired, &outline).await)
    );
}

#[tokio::test]
async fn a_discarded_outline_is_told_as_discarded() {
    // given: an outline with a chapter
    let wired = a_workbench().await;
    let outline = an_outline(&wired).await;
    a_section(&wired, &outline, None, None, "Morning").await;
    wired.settle().await;

    // when: its project is swept away
    wired
        .outlines
        .sweep("project_1")
        .await
        .expect("sweeping should succeed");
    wired.settle().await;

    // then: the last state told says it is discarded
    assert!(wired.states.last().discarded);
}

fn added_at(outline: &OutlineId, version: u64) -> Message {
    eventpublishing::published(
        SECTION_ADDED,
        "outline",
        &outline.to_string(),
        version,
        json!({ "version": 0, "name": "SECTION_ADDED" }),
        &Agent::System,
        clock::at(1_000),
    )
}

async fn told_on_hearing(wired: &Wired, message: Message) -> usize {
    let clock = Arc::new(FixedClock::new(clock::at(1_000)));
    let outbox = Arc::new(InMemoryOutbox::new(
        Arc::new(FakePublisher::default()),
        clock.clone(),
    ));
    let telling = Inboxed::new(
        TellStateListener::new(wired.outlines.clone(), outbox.clone(), clock.clone()),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryInbox::new(clock)),
    );

    telling
        .handle(&message)
        .await
        .expect("the listener should take this message");

    outbox.waiting_to_be_published()
}

#[tokio::test]
async fn an_event_older_than_the_outline_tells_nothing_as_a_later_one_will() {
    // given: an outline changed since it was started
    let wired = a_workbench().await;
    let outline = an_outline(&wired).await;
    a_section(&wired, &outline, None, None, "Morning").await;
    let now = version_of(&wired, &outline).await;

    // expect: an older event tells nothing, and the newest tells the state
    assert_eq!(
        told_on_hearing(&wired, added_at(&outline, now - 1)).await,
        0
    );
    assert_eq!(told_on_hearing(&wired, added_at(&outline, now)).await, 1);
}

#[test]
fn it_hears_every_outline_event_but_not_its_own_states() {
    // given: what the listener hears
    let hears = |routing: &str| {
        <TellStateListener as inbox::KeptHandler>::LISTENS_TO
            .iter()
            .any(|pattern| {
                messaging::Subscription::parse(pattern)
                    .expect("a declared pattern")
                    .covers(&messaging::RoutingKey::parse(routing).expect("a declared key"))
            })
    };

    // expect: every outline event, and not the state it tells, or it would tell forever
    for routing in [
        outline_contract::STARTED,
        outline_contract::SECTION_ADDED,
        outline_contract::SECTION_RETITLED,
        outline_contract::SECTION_MOVED,
        outline_contract::SECTION_PROMOTED,
        outline_contract::SECTION_DEMOTED,
        outline_contract::SECTION_REMOVED,
        outline_contract::ATTACHED,
        outline_contract::DETACHED,
        outline_contract::DISCARDED,
    ] {
        assert!(hears(routing), "{routing}");
    }
    assert!(!hears(outline_contract::STATE_CHANGED));
}
