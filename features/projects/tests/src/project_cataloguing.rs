use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{Agent, AggregateId, EventMetadata, Recorded, Version};
use messaging::{Message, RoutingKey};
use projects_core::{KIND, ProjectCatalog, ProjectEvent, ProjectId, ProjectName};
use projects_messaging::message_for;
use serde_json::json;
use time::{Duration, OffsetDateTime};

use crate::wiring::{Wired, wired};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn an_author() -> Agent {
    Agent::Anonymous
}

fn a_workbench() -> Wired {
    wired(Arc::new(FixedClock::new(at(1_000))))
}

async fn listed(wired: &Wired) -> Vec<String> {
    wired
        .catalog
        .all()
        .await
        .expect("looking should succeed")
        .into_iter()
        .map(|summary| summary.name.to_string())
        .collect()
}

async fn a_started_project(wired: &Wired) -> ProjectId {
    let id = wired
        .projects
        .start("The Weaver's Apprentice", &an_author())
        .await
        .expect("starting should succeed");
    wired.settle().await;

    id
}

fn a_name() -> ProjectName {
    ProjectName::new("The Weaver's Apprentice").expect("a plain name is fine")
}

fn message_about(id: &ProjectId, event: ProjectEvent) -> Message {
    let happened = Recorded {
        metadata: EventMetadata {
            aggregate: AggregateId::from(id),
            kind: KIND,
            version: Version::of(1),
            agent: an_author(),
            occurred_at: at(1_000),
            is_snapshot: false,
        },
        event,
    };

    message_for(&happened).expect("this event should be published")
}

fn a_started_project_told_of(id: &ProjectId) -> Message {
    message_about(id, ProjectEvent::Started(a_name()))
}

#[tokio::test]
async fn a_started_project_reaches_the_listing() {
    let wired = a_workbench();

    a_started_project(&wired).await;

    assert_eq!(listed(&wired).await, ["The Weaver's Apprentice"]);
}

#[tokio::test]
async fn the_listing_carries_the_moments_the_project_stands_at() {
    let wired = a_workbench();

    a_started_project(&wired).await;

    let found = wired.catalog.all().await.expect("looking should succeed");
    assert_eq!(
        found
            .first()
            .map(|listed| (listed.created_at, listed.updated_at)),
        Some((at(1_000), at(1_000))),
        "the listing is what an author sees, and it says when they last touched each project"
    );
}

#[tokio::test]
async fn hearing_the_same_message_twice_leaves_one_project() {
    let wired = a_workbench();
    let id = a_started_project(&wired).await;

    wired
        .projector
        .handle(&a_started_project_told_of(&id))
        .await
        .expect("a redelivery is not a failure");

    assert_eq!(
        listed(&wired).await,
        ["The Weaver's Apprentice"],
        "a broker redelivers, so hearing twice must count once"
    );
}

#[tokio::test]
async fn a_stale_message_cannot_resurrect_a_deleted_project() {
    let wired = a_workbench();
    let id = a_started_project(&wired).await;
    wired
        .projects
        .delete(&id.to_string(), None, &an_author())
        .await
        .expect("deleting should succeed");
    wired.settle().await;
    assert!(
        listed(&wired).await.is_empty(),
        "the deletion was projected"
    );

    wired
        .projector
        .handle(&a_started_project_told_of(&id))
        .await
        .expect("hearing a stale message is not a failure");

    assert!(
        listed(&wired).await.is_empty(),
        "the projector reads the project rather than trusting the message, so order cannot bite"
    );
}

#[tokio::test]
async fn the_listing_follows_the_latest_name_however_messages_arrive() {
    let wired = a_workbench();
    let id = a_started_project(&wired).await;
    wired
        .projects
        .rename(&id.to_string(), "A crown of straw", None, &an_author())
        .await
        .expect("renaming should succeed");
    wired.settle().await;

    wired
        .projector
        .handle(&a_started_project_told_of(&id))
        .await
        .expect("hearing a stale message is not a failure");

    assert_eq!(
        listed(&wired).await,
        ["A crown of straw"],
        "an out of order redelivery must not roll the listing back"
    );
}

#[tokio::test]
async fn a_message_about_no_project_at_all_is_refused() {
    let wired = a_workbench();
    let nonsense = Message::opening(
        RoutingKey::parse("project.started").expect("a plain key is fine"),
        json!({ "nothing": "useful" }),
        at(1_000),
    );

    let refused = wired.projector.handle(&nonsense).await;

    assert!(
        refused.is_err(),
        "a message the projector cannot act on belongs in dead letters, not silently dropped"
    );
}

#[tokio::test]
async fn a_message_about_a_project_that_was_never_stored_is_refused() {
    let wired = a_workbench();
    let never_stored = ProjectId::generate(at(1_000));

    let refused = wired
        .projector
        .handle(&a_started_project_told_of(&never_stored))
        .await;

    assert!(
        refused.is_err(),
        "a refusal reaches dead letters and can be retried, while acknowledging it would \
         drop the project from the listing for good"
    );
}
