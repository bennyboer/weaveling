use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{Agent, AggregateContext, AggregateId, EventMetadata, Recorded, Version};
use ideas_core::{IdeaCatalog, IdeaEvent, IdeaId, IdeaTitle, KIND, ProjectLink};
use messaging::{Message, RoutingKey};
use serde_json::json;
use time::{Duration, OffsetDateTime};

use crate::wiring;
use crate::wiring::Wired;

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn an_author() -> Agent {
    Agent::Local
}

fn a_workbench() -> Wired {
    wiring::wired(Arc::new(FixedClock::new(at(1_000))))
}

async fn titles_listed(wired: &Wired) -> Vec<String> {
    wired
        .catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("looking should succeed")
        .into_iter()
        .map(|summary| summary.title.to_string())
        .collect()
}

async fn a_captured_idea(wired: &Wired) -> IdeaId {
    let id = wired
        .ideas
        .capture("project_1", "The Loom", &an_author())
        .await
        .expect("capturing should succeed")
        .0;
    wired.settle().await;

    id
}

fn a_title() -> IdeaTitle {
    IdeaTitle::new("The Loom").expect("a plain title is fine")
}

fn message_about(id: &IdeaId, event: IdeaEvent) -> Message {
    let happened = Recorded {
        metadata: EventMetadata {
            aggregate: AggregateId::from(id),
            kind: KIND,
            version: Version::of(1),
            agent: an_author(),
            occurred_at: at(1_000),
            is_snapshot: false,
            context: AggregateContext::default(),
        },
        event,
    };

    ideas_messaging::message_for(&happened).expect("this event should be published")
}

fn a_captured_idea_told_of(id: &IdeaId) -> Message {
    message_about(
        id,
        IdeaEvent::Captured {
            project: ProjectLink::from("project_1"),
            title: a_title(),
        },
    )
}

#[tokio::test]
async fn a_captured_idea_reaches_the_listing() {
    // given: a workbench
    let wired = a_workbench();

    // when: an idea is captured
    a_captured_idea(&wired).await;

    // then: it is listed
    assert_eq!(titles_listed(&wired).await, ["The Loom"]);
}

#[tokio::test]
async fn hearing_the_same_message_twice_leaves_one_idea() {
    // given: a captured idea
    let wired = a_workbench();
    let id = a_captured_idea(&wired).await;

    // when: its capture is heard again
    wired
        .projector
        .handle(&a_captured_idea_told_of(&id))
        .await
        .expect("a redelivery is not a failure");

    // then: it is listed once
    assert_eq!(
        titles_listed(&wired).await,
        ["The Loom"],
        "a broker redelivers, so hearing twice must count once"
    );
}

#[tokio::test]
async fn a_stale_message_cannot_resurrect_a_discarded_idea() {
    // given: a discarded idea
    let wired = a_workbench();
    let id = a_captured_idea(&wired).await;
    wired
        .ideas
        .discard(&id.to_string(), None, &an_author())
        .await
        .expect("discarding should succeed");
    wired.settle().await;
    // and: it is no longer listed
    assert!(
        titles_listed(&wired).await.is_empty(),
        "the discard was projected"
    );

    // when: its capture is heard again
    wired
        .projector
        .handle(&a_captured_idea_told_of(&id))
        .await
        .expect("hearing a stale message is not a failure");

    // then: it stays unlisted
    assert!(
        titles_listed(&wired).await.is_empty(),
        "the projector reads the idea rather than trusting the message, so order cannot bite"
    );
    // and: its tombstone is forgotten once, at its time
    assert_eq!(
        (
            wired
                .ideas
                .forget_tombstones_laid_before(at(1_000), 10)
                .await
                .expect("forgetting should succeed"),
            wired
                .ideas
                .forget_tombstones_laid_before(at(1_001), 10)
                .await
                .expect("forgetting should succeed"),
            wired
                .ideas
                .forget_tombstones_laid_before(at(1_001), 10)
                .await
                .expect("forgetting should succeed"),
        ),
        (0, 1, 0),
        "its tombstone was laid as it was catalogued gone, by the clock, and forgotten for good"
    );
}

#[tokio::test]
async fn the_listing_follows_the_latest_title_however_messages_arrive() {
    // given: a retitled idea
    let wired = a_workbench();
    let id = a_captured_idea(&wired).await;
    wired
        .ideas
        .retitle(&id.to_string(), "The Silent Loom", None, &an_author())
        .await
        .expect("retitling should succeed");
    wired.settle().await;

    // when: its capture is heard again
    wired
        .projector
        .handle(&a_captured_idea_told_of(&id))
        .await
        .expect("hearing a stale message is not a failure");

    // then: its latest title is listed
    assert_eq!(
        titles_listed(&wired).await,
        ["The Silent Loom"],
        "an out of order redelivery must not roll the listing back"
    );
}

#[tokio::test]
async fn a_message_about_no_idea_at_all_is_refused() {
    // given: a message about no idea at all
    let wired = a_workbench();
    let nonsense = Message::opening(
        RoutingKey::parse("idea.captured").expect("a plain key is fine"),
        json!({ "nothing": "useful" }),
        at(1_000),
    );

    // when: it is heard
    let refused = wired.projector.handle(&nonsense).await;

    // then: it is refused
    assert!(
        refused.is_err(),
        "a message the projector cannot act on belongs in dead letters, not silently dropped"
    );
}

#[tokio::test]
async fn a_message_about_an_idea_that_was_never_stored_is_refused() {
    // given: an idea never stored
    let wired = a_workbench();
    let never_stored = IdeaId::generate(at(1_000));

    // when: its capture is heard
    let refused = wired
        .projector
        .handle(&a_captured_idea_told_of(&never_stored))
        .await;

    // then: it is refused
    assert!(
        refused.is_err(),
        "a refusal reaches dead letters and can be retried, while acknowledging it would drop the \
         idea from the listing for good"
    );
}
