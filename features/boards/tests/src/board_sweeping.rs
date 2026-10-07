use std::sync::Arc;

use boards_core::BoardCatalog;
use boards_messaging::DiscardBoardsOnProjectDeleted;
use clock::FixedClock;
use eventsourcing::{Agent, AgentId};
use messaging::{Listener, Message, RoutingKey};
use projects_contract::DELETED;
use serde_json::json;
use time::{Duration, OffsetDateTime};

use crate::wiring::{Wired, wired};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn an_author() -> Agent {
    Agent::User(AgentId::from("author-7"))
}

fn a_workbench() -> (Wired, DiscardBoardsOnProjectDeleted) {
    let wired = wired(Arc::new(FixedClock::new(at(1_000))));
    let sweep = DiscardBoardsOnProjectDeleted::new(wired.boards.clone());

    (wired, sweep)
}

fn deleted(project: &str) -> Message {
    Message::opening(
        RoutingKey::parse(DELETED).expect("a declared routing key is fine"),
        json!({
            "event": { "version": 0, "name": "DELETED" },
            "aggregate": { "id": project, "kind": "project", "version": 3 },
            "agent": { "kind": "anonymous", "id": null },
            "occurred_at": "1970-01-01T00:16:40Z",
        }),
        at(1_000),
    )
}

#[tokio::test]
async fn a_board_the_catalog_has_not_heard_of_yet_is_still_swept() {
    let (wired, sweep) = a_workbench();
    let opened = wired
        .boards
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed")
        .id;
    assert!(
        wired
            .catalog
            .in_project(&"project_1".into())
            .await
            .expect("asking should succeed")
            .is_empty(),
        "nothing has relayed the start yet, so the catalog cannot know this board"
    );

    sweep
        .handle(&deleted("project_1"))
        .await
        .expect("the sweep should take the deletion");

    let standing = wired
        .boards
        .get(&opened.to_string())
        .await
        .expect("the board is still there to read");
    assert!(
        standing.state.is_discarded(),
        "a board opened just before its project was deleted must not outlive it"
    );
}

#[tokio::test]
async fn a_project_that_never_opened_a_board_is_swept_without_complaint() {
    let (_wired, sweep) = a_workbench();

    sweep
        .handle(&deleted("project_1"))
        .await
        .expect("nothing to discard is not a failure");
}

#[tokio::test]
async fn hearing_the_same_deletion_twice_is_harmless() {
    let (wired, sweep) = a_workbench();
    wired
        .boards
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed");

    for _ in 0..2 {
        sweep
            .handle(&deleted("project_1"))
            .await
            .expect("a redelivered deletion should be taken again");
    }
}

#[tokio::test]
async fn only_the_deleted_project_loses_its_board() {
    let (wired, sweep) = a_workbench();
    let kept = wired
        .boards
        .open("project_2", &an_author())
        .await
        .expect("opening should succeed")
        .id;

    sweep
        .handle(&deleted("project_1"))
        .await
        .expect("the sweep should take the deletion");

    assert!(
        !wired
            .boards
            .get(&kept.to_string())
            .await
            .expect("the other board is there to read")
            .state
            .is_discarded()
    );
}
