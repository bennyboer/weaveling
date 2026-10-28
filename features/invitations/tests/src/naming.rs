use eventsourcing::Version;
use invitations_core::{InvitationCatalog, ProjectLink, ProjectName};
use invitations_messaging::RememberProjectNamesListener;
use messaging::{Handler, Listener, Message, RoutingKey};
use projects_contract::{DELETED, DETAILS_CHANGED, RENAMED, STARTED};
use serde_json::{Value, json};
use time::{Duration, OffsetDateTime};

use crate::wiring;
use crate::wiring::Wired;

fn told(routing: &str, version: u64, event: Value, at: OffsetDateTime) -> Message {
    Message::opening(
        RoutingKey::parse(routing).expect("a declared routing key is fine"),
        json!({
            "event": event,
            "aggregate": { "id": "project_loom", "kind": "project", "version": version },
            "agent": { "kind": "account", "id": "account_ada" },
            "occurred_at": "2023-11-14T22:13:20Z",
        }),
        at,
    )
}

fn started(version: u64, name: &str) -> Message {
    told(
        STARTED,
        version,
        json!({ "version": 0, "name": "STARTED", "project_name": name }),
        OffsetDateTime::UNIX_EPOCH,
    )
}

fn renamed(version: u64, name: &str) -> Message {
    told(
        RENAMED,
        version,
        json!({ "version": 0, "name": "RENAMED", "project_name": name }),
        OffsetDateTime::UNIX_EPOCH,
    )
}

fn deleted(version: u64, at: OffsetDateTime) -> Message {
    told(
        DELETED,
        version,
        json!({ "version": 0, "name": "DELETED" }),
        at,
    )
}

async fn named(wired: &Wired) -> Vec<ProjectName> {
    wired
        .catalog
        .project_names(&[ProjectLink::from("project_loom")])
        .await
        .expect("asking should succeed")
}

async fn heard(listener: &RememberProjectNamesListener, messages: &[Message]) {
    for message in messages {
        listener
            .take_in(message)
            .await
            .expect("the listener takes it in");
    }
}

#[tokio::test]
async fn a_project_is_known_by_the_name_it_was_started_with_and_then_by_its_newest() {
    let wired = wiring::wired();
    let listener = RememberProjectNamesListener::new(wired.invitations.clone());
    for routing in [STARTED, RENAMED, DELETED] {
        assert!(listener.hears(&RoutingKey::parse(routing).expect("a declared key")));
    }

    heard(&listener, &[started(1, "The Loom")]).await;
    assert_eq!(
        named(&wired).await,
        vec![ProjectName {
            project: ProjectLink::from("project_loom"),
            version: Version::of(1),
            name: "The Loom".to_owned(),
        }]
    );

    heard(
        &listener,
        &[renamed(3, "The Silent Loom"), renamed(2, "The Quiet Loom")],
    )
    .await;
    assert_eq!(
        named(&wired).await,
        vec![ProjectName {
            project: ProjectLink::from("project_loom"),
            version: Version::of(3),
            name: "The Silent Loom".to_owned(),
        }],
        "a rename heard late changes nothing"
    );
}

#[tokio::test]
async fn a_deleted_project_is_named_no_more_and_its_tombstone_kept_until_forgotten() {
    let wired = wiring::wired();
    let listener = RememberProjectNamesListener::new(wired.invitations.clone());
    let gone = OffsetDateTime::UNIX_EPOCH + Duration::days(10);

    heard(
        &listener,
        &[started(1, "The Loom"), deleted(4, gone), renamed(3, "Late")],
    )
    .await;

    assert!(named(&wired).await.is_empty());
    assert_eq!(
        wired
            .invitations
            .forget_tombstones_laid_before(gone, 10)
            .await
            .expect("forgetting should succeed"),
        0,
        "the tombstone was laid when the project was deleted"
    );
    assert_eq!(
        wired
            .invitations
            .forget_tombstones_laid_before(gone + Duration::SECOND, 10)
            .await
            .expect("forgetting should succeed"),
        1
    );
}

#[tokio::test]
async fn what_tells_no_name_nor_a_deletion_is_refused() {
    let wired = wiring::wired();
    let listener = RememberProjectNamesListener::new(wired.invitations.clone());

    for unreadable in [
        told(
            DETAILS_CHANGED,
            2,
            json!({ "version": 0, "name": "DETAILS_CHANGED", "description": "" }),
            OffsetDateTime::UNIX_EPOCH,
        ),
        told(
            STARTED,
            1,
            json!({ "version": 0, "name": "STARTED" }),
            OffsetDateTime::UNIX_EPOCH,
        ),
    ] {
        assert!(
            listener.take_in(&unreadable).await.is_err(),
            "a message the listener cannot act on belongs in dead letters"
        );
    }
    assert!(named(&wired).await.is_empty());
}
