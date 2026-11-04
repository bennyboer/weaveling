use std::sync::Arc;

use authorizing::AllowingEverything;
use clock::FixedClock;
use eventsourcing::{
    AccountId, Agent, Aggregate, AggregateContext, AggregateId, EventMetadata, EventStore,
    FailsAppending, Recorded, RefusesAppending, ServiceError, Standing, StoreError, Version,
};
use messaging::Message;
use outline_core::{
    Attachment, KIND, Outline, OutlineCatalog, OutlineEvent, OutlineId, OutlinesCore,
    OutlinesError, ProjectLink, SectionId, SectionTitle,
};
use time::{Duration, OffsetDateTime};

use registry::Registry;
use transacting::{InMemoryTransactions, Transactions};

use crate::shapes;
use crate::wiring;
use crate::wiring::Wired;

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn an_author() -> Agent {
    Agent::Account(AccountId::from("author-7"))
}

fn a_workbench() -> Wired {
    wiring::wired(Arc::new(FixedClock::new(at(1_000))))
}

fn a_title(what: &str) -> SectionTitle {
    SectionTitle::new(what).expect("the title should be usable")
}

fn recorded(id: &OutlineId, event: OutlineEvent, version: u64) -> Recorded<OutlineEvent> {
    Recorded {
        metadata: EventMetadata {
            aggregate: AggregateId::from(id),
            kind: KIND,
            version: Version::of(version),
            agent: an_author(),
            occurred_at: at(1_000),
            is_snapshot: false,
            context: AggregateContext::default(),
        },
        event,
    }
}

fn a_start(id: &OutlineId, project: &str) -> Recorded<OutlineEvent> {
    recorded(
        id,
        OutlineEvent::Started {
            project: ProjectLink::from(project),
        },
        1,
    )
}

fn told_of(happened: &Recorded<OutlineEvent>) -> Message {
    outline_messaging::message_for(happened).expect("a start should be published")
}

async fn a_stored_outline(wired: &Wired, project: &str) -> OutlineId {
    let id = OutlineId::generate(at(1_000));
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    wired
        .store
        .append(
            &mut transaction,
            &AggregateId::from(&id),
            KIND,
            Version::ZERO,
            &[a_start(&id, project)],
        )
        .await
        .expect("appending should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    id
}

async fn catalogued(wired: &Wired, project: &str) -> Vec<OutlineId> {
    wired
        .catalog
        .in_project(&ProjectLink::from(project))
        .await
        .expect("listing should succeed")
        .into_iter()
        .map(|summary| summary.id)
        .collect()
}

async fn a_written_book(wired: &Wired) -> (OutlineId, Vec<SectionId>) {
    let outline = wired
        .outlines
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed")
        .id;
    let address = outline.to_string();
    let mut added = Vec::new();

    let part = wired
        .outlines
        .add(
            &address,
            None,
            None,
            a_title("Part One"),
            None,
            &an_author(),
        )
        .await
        .expect("adding should succeed")
        .section;
    added.push(part);

    let mut behind = None;
    for named in ["Chapter 1", "Chapter 2", "Chapter 3"] {
        let chapter = wired
            .outlines
            .add(
                &address,
                Some(part),
                behind,
                a_title(named),
                None,
                &an_author(),
            )
            .await
            .expect("adding should succeed")
            .section;
        added.push(chapter);
        behind = Some(chapter);
    }

    for (nth, chapter) in added[1..].iter().enumerate() {
        wired
            .outlines
            .attach(
                &address,
                Attachment::scene(&format!("scene_{nth}")),
                *chapter,
                None,
                None,
                &an_author(),
            )
            .await
            .expect("attaching should succeed");
    }

    wired
        .outlines
        .promote(&address, added[2], None, &an_author())
        .await
        .expect("promoting should succeed");

    (outline, added)
}

fn shape(outline: &Outline) -> String {
    outline
        .sections()
        .into_iter()
        .map(|placed| {
            let mut depth = 0;
            let mut walking = placed.parent;

            while let Some(here) = walking {
                depth += 1;
                walking = outline.parent_of(&here);
            }

            format!("{}{}", "  ".repeat(depth), placed.title)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn standing(wired: &Wired, outline: &OutlineId) -> Standing<Outline> {
    wired
        .outlines
        .read(&outline.to_string(), &an_author())
        .await
        .expect("reading should succeed")
}

#[tokio::test]
async fn starting_an_outline_puts_it_in_the_catalog() {
    // given: a project with no outline
    let wired = a_workbench();

    // when: its outline is opened
    let outline = wired
        .outlines
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed")
        .id;
    wired.settle().await;

    // then: the catalog lists it in the project
    assert_eq!(catalogued(&wired, "project_1").await, vec![outline]);
}

#[tokio::test]
async fn a_start_the_projector_never_heard_is_catalogued_when_it_arrives() {
    // given: an outline stored without its start being heard
    let wired = a_workbench();
    let outline = a_stored_outline(&wired, "project_1").await;

    // when: its start is heard
    wired
        .projector
        .handle(&told_of(&a_start(&outline, "project_1")))
        .await
        .expect("projecting should succeed");

    // then: the catalog lists it in its project
    assert_eq!(
        catalogued(&wired, "project_1").await,
        vec![outline],
        "a projection is built from the log, not from whoever happened to make the call"
    );
}

#[tokio::test]
async fn hearing_the_same_start_twice_leaves_one_entry() {
    // given: a stored outline
    let wired = a_workbench();
    let outline = a_stored_outline(&wired, "project_1").await;

    // when: its start is heard twice
    for _ in 0..2 {
        wired
            .projector
            .handle(&told_of(&a_start(&outline, "project_1")))
            .await
            .expect("a redelivery is not a failure");
    }

    // then: the catalog lists it once
    assert_eq!(
        catalogued(&wired, "project_1").await.len(),
        1,
        "a broker redelivers, so a projector has to be idempotent"
    );
}

#[tokio::test]
async fn a_start_naming_an_outline_that_was_never_stored_is_refused() {
    // given: an outline never stored
    let wired = a_workbench();
    let never_written = OutlineId::generate(at(2_000));

    // then: hearing its start fails
    assert!(
        wired
            .projector
            .handle(&told_of(&a_start(&never_written, "project_1")))
            .await
            .is_err(),
        "a projector that cannot read the stream it was told about belongs in dead letters"
    );
}

#[tokio::test]
async fn a_service_that_never_saw_the_writes_reads_the_same_book_out_of_the_log() {
    // given: a written book
    let wired = a_workbench();
    let (outline, _) = a_written_book(&wired).await;

    // when: it is read by a service that never saw it written
    let replayed = (wired.reopened)()
        .read(&outline.to_string(), &an_author())
        .await
        .expect("a fresh service should read the stream");

    // then: it has the same shape
    assert_eq!(
        shape(&replayed.state),
        shapes::shaped(
            "
            Part One
              Chapter 1
            Chapter 2
              Chapter 3
        "
        ),
        "structure has to come back out of the log, not out of whoever wrote it"
    );
    // and: reads in the same order
    assert_eq!(
        replayed
            .state
            .reading_order()
            .iter()
            .map(|scene| scene.to_string())
            .collect::<Vec<_>>(),
        vec!["scene_0", "scene_1", "scene_2"],
        "and the book has to read in the same order it did before the promotion"
    );
}

#[tokio::test]
async fn a_snapshot_carries_the_whole_shape_of_the_book() {
    // given: a written book
    let wired = a_workbench();
    let (outline, _) = a_written_book(&wired).await;
    let built = standing(&wired, &outline).await;

    // when: it is snapshotted
    let OutlineEvent::Snapshotted { sections, .. } = built.state.snapshot() else {
        panic!("a snapshot should be a snapshot");
    };

    // then: the snapshot holds every section as it stands
    assert_eq!(
        sections,
        built.state.sections(),
        "a snapshot is what the log collapses to, so it has to say everything the fold knows"
    );
    // and: keeps them nested
    assert!(
        sections.iter().any(|placed| placed.parent.is_some()),
        "a flat snapshot of a nested book would lose the nesting silently"
    );
}

#[tokio::test]
async fn opening_a_project_whose_outline_is_already_claimed_finds_it() {
    // given: a project whose outline is claimed and stored
    let wired = a_workbench();
    let known = OutlineId::generate(at(500));
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    wired
        .registry
        .claim(
            &mut transaction,
            KIND.as_str(),
            "project_1",
            &known.to_string(),
        )
        .await
        .expect("claiming should succeed");
    wired
        .store
        .append(
            &mut transaction,
            &AggregateId::from(&known),
            KIND,
            Version::ZERO,
            &[a_start(&known, "project_1")],
        )
        .await
        .expect("appending should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    // when: the project's outline is opened
    let opened = wired
        .outlines
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed");

    // then: it is the claimed one
    assert_eq!(
        opened.id, known,
        "the find branch takes whoever holds the claim rather than starting afresh; the claim is \
         authoritative where the catalog is only a projection"
    );
}

#[tokio::test]
async fn an_outline_whose_start_cannot_be_kept_leaves_no_claim_behind_and_is_started_next_time() {
    // given: a store that refuses every start
    let wired = a_workbench();
    let refusing = OutlinesCore::new(
        Arc::new(RefusesAppending(wired.store.clone())),
        Arc::new(InMemoryTransactions::new()),
        wired.catalog.clone(),
        wired.registry.clone(),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(at(1_000))),
    );

    // when: an outline is opened against it
    let refused = refusing.open("project_1", &an_author()).await;
    // and: then opened again against one that keeps it
    let held_after_refusal = wired
        .registry
        .holder(KIND.as_str(), "project_1")
        .await
        .expect("asking should succeed");
    let opened = wired
        .outlines
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed")
        .id;

    // then: the first opening fails
    assert!(refused.is_err());
    // and: leaves no claim behind
    assert_eq!(
        held_after_refusal, None,
        "a claim kept for an outline that never started would point every later open at nothing"
    );
    // and: the second claims the project for its outline
    assert_eq!(
        wired
            .registry
            .holder(KIND.as_str(), "project_1")
            .await
            .expect("asking should succeed"),
        Some(opened.to_string())
    );
}

async fn a_section_added_while_losing_races(
    losses: u8,
    expected: Option<Version>,
) -> (Result<Version, OutlinesError>, usize) {
    let wired = a_workbench();
    let outline = wired
        .outlines
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed")
        .id;
    let losing = OutlinesCore::new(
        Arc::new(FailsAppending::losing_races(wired.store.clone(), losses)),
        Arc::new(InMemoryTransactions::new()),
        wired.catalog.clone(),
        wired.registry.clone(),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(at(1_000))),
    );

    let added = losing
        .add(
            &outline.to_string(),
            None,
            None,
            a_title("Act One"),
            expected,
            &an_author(),
        )
        .await
        .map(|added| added.version);
    let sections = wired
        .outlines
        .read(&outline.to_string(), &an_author())
        .await
        .expect("the outline is there")
        .state
        .sections()
        .len();

    (added, sections)
}

#[tokio::test]
async fn a_section_added_after_a_lost_race_is_tried_again_and_lands() {
    // when: a section is added after losing four races
    let (added, sections) = a_section_added_while_losing_races(4, None).await;

    // then: it lands
    assert_eq!(added.expect("the fifth try lands"), Version::of(2));
    assert_eq!(sections, 1);
}

#[tokio::test]
async fn a_section_added_after_losing_every_race_it_may_try_is_a_conflict() {
    // when: a section is added after losing as many races as it may try
    let (added, sections) = a_section_added_while_losing_races(5, None).await;

    // then: it is refused as outdated
    assert!(matches!(
        added,
        Err(OutlinesError::Events(ServiceError::Store(
            StoreError::Outdated { .. }
        )))
    ));
    // and: the outline holds no section
    assert_eq!(sections, 0);
}

#[tokio::test]
async fn a_section_added_at_a_version_after_a_lost_race_is_not_tried_again() {
    // when: a section is added at a version after losing a race
    let (added, sections) = a_section_added_while_losing_races(1, Some(Version::of(1))).await;

    // then: it is refused as outdated and the outline holds no section
    assert!(
        matches!(
            added,
            Err(OutlinesError::Events(ServiceError::Store(
                StoreError::Outdated { .. }
            )))
        ) && sections == 0,
        "whoever named a version hears that it moved on, a 412, rather than having it papered over"
    );
}
