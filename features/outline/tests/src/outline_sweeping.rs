use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{Agent, AgentId};
use messaging::{Listener, Message, RoutingKey};
use outline_core::OutlineCatalog;
use outline_messaging::DiscardOutlinesOnProjectDeleted;
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

fn a_workbench() -> (Wired, DiscardOutlinesOnProjectDeleted) {
    let wired = wired(Arc::new(FixedClock::new(at(1_000))));
    let sweep = DiscardOutlinesOnProjectDeleted::new(wired.outlines.clone());

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
async fn a_outline_the_catalog_has_not_heard_of_yet_is_still_swept() {
    let (wired, sweep) = a_workbench();
    let opened = wired
        .outlines
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
        "nothing has relayed the start yet, so the catalog cannot know this outline"
    );

    sweep
        .handle(&deleted("project_1"))
        .await
        .expect("the sweep should take the deletion");

    let standing = wired
        .outlines
        .get(&opened.to_string())
        .await
        .expect("the outline is still there to read");
    assert!(
        standing.state.is_discarded(),
        "a outline opened just before its project was deleted must not outlive it"
    );
}

#[tokio::test]
async fn a_project_that_never_opened_an_outline_is_swept_without_complaint() {
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
        .outlines
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
async fn only_the_deleted_project_loses_its_outline() {
    let (wired, sweep) = a_workbench();
    let kept = wired
        .outlines
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
            .outlines
            .get(&kept.to_string())
            .await
            .expect("the other outline is there to read")
            .state
            .is_discarded()
    );
}
