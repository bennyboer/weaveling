use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{
    Agent, AgentId, Aggregate, AggregateId, EventMetadata, EventStore, Recorded, Standing, Version,
};
use messaging::Message;
use outline_core::{
    KIND, Outline, OutlineCatalog, OutlineEvent, OutlineId, PieceLink, ProjectLink, SectionId,
    SectionTitle,
};
use outline_messaging::message_for;
use time::{Duration, OffsetDateTime};

use crate::shapes::shaped;
use crate::wiring::{Wired, wired};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn an_author() -> Agent {
    Agent::User(AgentId::from("author-7"))
}

fn a_workbench() -> Wired {
    wired(Arc::new(FixedClock::new(at(1_000))))
}

fn titled(what: &str) -> SectionTitle {
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
    message_for(happened).expect("a start should be published")
}

async fn a_stored_outline(wired: &Wired, project: &str) -> OutlineId {
    let id = OutlineId::generate(at(1_000));
    wired
        .store
        .append(
            &AggregateId::from(&id),
            KIND,
            Version::ZERO,
            &[a_start(&id, project)],
        )
        .await
        .expect("appending should succeed");

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
        .add(&address, None, None, titled("Part One"), None, &an_author())
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
                titled(named),
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
                PieceLink::from(format!("piece_{nth}").as_str()),
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
        .get(&outline.to_string())
        .await
        .expect("reading should succeed")
}

#[tokio::test]
async fn starting_an_outline_puts_it_in_the_catalog() {
    let wired = a_workbench();

    let outline = wired
        .outlines
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed")
        .id;

    assert_eq!(catalogued(&wired, "project_1").await, vec![outline]);
}

#[tokio::test]
async fn a_start_the_projector_never_heard_is_catalogued_when_it_arrives() {
    let wired = a_workbench();
    let outline = a_stored_outline(&wired, "project_1").await;

    wired
        .projector
        .handle(&told_of(&a_start(&outline, "project_1")))
        .await
        .expect("projecting should succeed");

    assert_eq!(
        catalogued(&wired, "project_1").await,
        vec![outline],
        "a projection is built from the log, not from whoever happened to make the call"
    );
}

#[tokio::test]
async fn hearing_the_same_start_twice_leaves_one_entry() {
    let wired = a_workbench();
    let outline = a_stored_outline(&wired, "project_1").await;

    for _ in 0..2 {
        wired
            .projector
            .handle(&told_of(&a_start(&outline, "project_1")))
            .await
            .expect("a redelivery is not a failure");
    }

    assert_eq!(
        catalogued(&wired, "project_1").await.len(),
        1,
        "a broker redelivers, so a projector has to be idempotent"
    );
}

#[tokio::test]
async fn a_start_naming_an_outline_that_was_never_stored_is_refused() {
    let wired = a_workbench();
    let never_written = OutlineId::generate(at(2_000));

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
    let wired = a_workbench();
    let (outline, _) = a_written_book(&wired).await;

    let replayed = (wired.reopened)()
        .get(&outline.to_string())
        .await
        .expect("a fresh service should read the stream");

    assert_eq!(
        shape(&replayed.state),
        shaped(
            "
            Part One
              Chapter 1
            Chapter 2
              Chapter 3
        "
        ),
        "structure has to come back out of the log, not out of whoever wrote it"
    );
    assert_eq!(
        replayed
            .state
            .reading_order()
            .iter()
            .map(|piece| piece.to_string())
            .collect::<Vec<_>>(),
        vec!["piece_0", "piece_1", "piece_2"],
        "and the book has to read in the same order it did before the promotion"
    );
}

#[tokio::test]
async fn a_snapshot_carries_the_whole_shape_of_the_book() {
    let wired = a_workbench();
    let (outline, _) = a_written_book(&wired).await;
    let built = standing(&wired, &outline).await;

    let OutlineEvent::Snapshotted { sections, .. } = built.state.snapshot() else {
        panic!("a snapshot should be a snapshot");
    };

    assert_eq!(
        sections,
        built.state.sections(),
        "a snapshot is what the log collapses to, so it has to say everything the fold knows"
    );
    assert!(
        sections.iter().any(|placed| placed.parent.is_some()),
        "a flat snapshot of a nested book would lose the nesting silently"
    );
}
