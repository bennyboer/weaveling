use std::sync::Arc;

use boards_core::{BoardCatalog, BoardEvent, BoardId, KIND, ProjectLink};
use boards_messaging::message_for;
use clock::FixedClock;
use eventsourcing::{Agent, AgentId, AggregateId, EventMetadata, EventStore, Recorded, Version};
use messaging::Message;
use time::{Duration, OffsetDateTime};

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

fn recorded(id: &BoardId, event: BoardEvent, version: u64) -> Recorded<BoardEvent> {
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

fn a_start(id: &BoardId, project: &str) -> Recorded<BoardEvent> {
    recorded(
        id,
        BoardEvent::Started {
            project: ProjectLink::from(project),
        },
        1,
    )
}

fn told_of(happened: &Recorded<BoardEvent>) -> Message {
    message_for(happened).expect("a start should be published")
}

async fn a_stored_board(wired: &Wired, project: &str) -> BoardId {
    let id = BoardId::generate(at(1_000));
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

async fn catalogued(wired: &Wired, project: &str) -> Vec<BoardId> {
    wired
        .catalog
        .in_project(&ProjectLink::from(project))
        .await
        .expect("looking should succeed")
        .into_iter()
        .map(|summary| summary.id)
        .collect()
}

#[tokio::test]
async fn hearing_that_a_board_started_catalogues_it() {
    let wired = a_workbench();
    let id = a_stored_board(&wired, "project_1").await;

    wired
        .projector
        .handle(&told_of(&a_start(&id, "project_1")))
        .await
        .expect("cataloguing should succeed");

    assert_eq!(catalogued(&wired, "project_1").await, vec![id]);
}

#[tokio::test]
async fn hearing_the_same_start_twice_catalogues_one_board() {
    let wired = a_workbench();
    let id = a_stored_board(&wired, "project_1").await;
    let told = told_of(&a_start(&id, "project_1"));

    wired
        .projector
        .handle(&told)
        .await
        .expect("cataloguing should succeed");
    wired
        .projector
        .handle(&told)
        .await
        .expect("a redelivery is not a failure");

    assert_eq!(
        catalogued(&wired, "project_1").await,
        vec![id],
        "a broker redelivers, so hearing twice must count once"
    );
}

#[tokio::test]
async fn each_project_is_catalogued_apart_from_the_others() {
    let wired = a_workbench();
    let mine = a_stored_board(&wired, "project_mine").await;
    let theirs = a_stored_board(&wired, "project_theirs").await;

    for (id, project) in [(mine, "project_mine"), (theirs, "project_theirs")] {
        wired
            .projector
            .handle(&told_of(&a_start(&id, project)))
            .await
            .expect("cataloguing should succeed");
    }

    assert_eq!(catalogued(&wired, "project_mine").await, vec![mine]);
    assert_eq!(catalogued(&wired, "project_theirs").await, vec![theirs]);
}

#[tokio::test]
async fn a_start_for_a_board_that_was_never_stored_is_refused() {
    let wired = a_workbench();
    let never_stored = BoardId::generate(at(1_000));

    let refused = wired
        .projector
        .handle(&told_of(&a_start(&never_stored, "project_1")))
        .await;

    assert!(
        refused.is_err(),
        "a refusal reaches dead letters and can be retried, while acknowledging it would          leave the catalog short a board for good"
    );
}
