use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{Agent, AggregateContext, AggregateId, EventMetadata, Recorded, Version};
use messaging::{Message, RoutingKey};
use projects_core::{KIND, ProjectCatalog, ProjectEvent, ProjectId, ProjectName, RawDetails};
use serde_json::json;

use crate::wiring;
use crate::wiring::Wired;

fn an_author() -> Agent {
    Agent::Local
}

fn a_workbench() -> Wired {
    wiring::wired(Arc::new(FixedClock::new(clock::at(1_000))))
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
    let (id, _) = wired
        .projects
        .start(
            "The Weaver's Apprentice",
            &RawDetails::default(),
            &an_author(),
        )
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
            occurred_at: clock::at(1_000),
            is_snapshot: false,
            context: AggregateContext::default(),
        },
        event,
    };

    projects_messaging::message_for(&happened).expect("this event should be published")
}

fn a_started_project_told_of(id: &ProjectId) -> Message {
    message_about(id, ProjectEvent::Started(a_name()))
}

#[tokio::test]
async fn a_started_project_reaches_the_listing() {
    // given: a workbench
    let wired = a_workbench();

    // when: a project is started
    a_started_project(&wired).await;

    // then: it is listed
    assert_eq!(listed(&wired).await, ["The Weaver's Apprentice"]);
}

#[tokio::test]
async fn the_listing_carries_the_moments_the_project_stands_at() {
    // given: a workbench
    let wired = a_workbench();

    // when: a project is started
    a_started_project(&wired).await;

    // then: it is listed as created and updated when it started
    let found = wired.catalog.all().await.expect("looking should succeed");
    assert_eq!(
        found
            .first()
            .map(|listed| (listed.created_at, listed.updated_at)),
        Some((clock::at(1_000), clock::at(1_000))),
        "the listing is what an author sees, and it says when they last touched each project"
    );
}

#[tokio::test]
async fn hearing_the_same_message_twice_leaves_one_project() {
    // given: a started project
    let wired = a_workbench();
    let id = a_started_project(&wired).await;

    // when: its start is heard again
    wired
        .projector
        .handle(&a_started_project_told_of(&id))
        .await
        .expect("a redelivery is not a failure");

    // then: it is listed once
    assert_eq!(
        listed(&wired).await,
        ["The Weaver's Apprentice"],
        "a broker redelivers, so hearing twice must count once"
    );
}

#[tokio::test]
async fn a_stale_message_cannot_resurrect_a_deleted_project() {
    // given: a trashed and purged project
    let wired = a_workbench();
    let id = a_started_project(&wired).await;
    wired
        .projects
        .trash(&id.to_string(), None, &an_author())
        .await
        .expect("trashing should succeed");
    wired
        .projects
        .purge(&id.to_string())
        .await
        .expect("purging should succeed");
    wired.settle().await;
    // and: it is no longer listed
    assert!(
        listed(&wired).await.is_empty(),
        "the deletion was projected"
    );

    // when: its start is heard late
    wired
        .projector
        .handle(&a_started_project_told_of(&id))
        .await
        .expect("hearing a stale message is not a failure");

    // then: it is still not listed
    assert!(
        listed(&wired).await.is_empty(),
        "the projector reads the project rather than trusting the message, so order cannot bite"
    );
    // and: its tombstone was laid as it went and is forgotten once
    assert_eq!(
        (
            wired
                .projects
                .forget_tombstones_laid_before(clock::at(1_000), 10)
                .await
                .expect("forgetting should succeed"),
            wired
                .projects
                .forget_tombstones_laid_before(clock::at(1_001), 10)
                .await
                .expect("forgetting should succeed"),
            wired
                .projects
                .forget_tombstones_laid_before(clock::at(1_001), 10)
                .await
                .expect("forgetting should succeed"),
        ),
        (0, 1, 0),
        "its tombstone was laid as it was catalogued gone, by the clock, and forgotten for good"
    );
}

#[tokio::test]
async fn the_listing_follows_the_latest_name_however_messages_arrive() {
    // given: a renamed project
    let wired = a_workbench();
    let id = a_started_project(&wired).await;
    wired
        .projects
        .rename(&id.to_string(), "A crown of straw", None, &an_author())
        .await
        .expect("renaming should succeed");
    wired.settle().await;

    // when: its start is heard late
    wired
        .projector
        .handle(&a_started_project_told_of(&id))
        .await
        .expect("hearing a stale message is not a failure");

    // then: it is listed by its latest name
    assert_eq!(
        listed(&wired).await,
        ["A crown of straw"],
        "an out of order redelivery must not roll the listing back"
    );
}

#[tokio::test]
async fn a_message_about_no_project_at_all_is_refused() {
    // given: a message about no project
    let wired = a_workbench();
    let nonsense = Message::opening(
        RoutingKey::parse("project.started").expect("a plain key is fine"),
        json!({ "nothing": "useful" }),
        clock::at(1_000),
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
async fn a_message_about_a_project_that_was_never_stored_is_refused() {
    // given: a project never stored
    let wired = a_workbench();
    let never_stored = ProjectId::generate(clock::at(1_000));

    // when: its start is heard
    let refused = wired
        .projector
        .handle(&a_started_project_told_of(&never_stored))
        .await;

    // then: it is refused
    assert!(
        refused.is_err(),
        "a refusal reaches dead letters and can be retried, while acknowledging it would \
         drop the project from the listing for good"
    );
}
