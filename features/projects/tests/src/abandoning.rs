use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{Agent, ServiceError};
use members_contract::ABANDONED;
use messaging::{Message, RoutingKey};
use projects_core::{ProjectError, ProjectsError, RawDetails};
use serde_json::json;
use time::{Duration, OffsetDateTime};
use trash::Trash;

use crate::wiring::{self, Wired};

fn bench() -> Wired {
    wiring::wired(Arc::new(FixedClock::new(clock::at(1_000))))
}

async fn a_project(wired: &Wired) -> String {
    let id = wired
        .projects
        .start("Tapestry", &RawDetails::default(), &Agent::Local)
        .await
        .expect("starting should succeed")
        .0
        .to_string();
    wired.settle().await;

    id
}

fn abandoned(project: &str, name: &str) -> Message {
    eventpublishing::published(
        ABANDONED,
        "members",
        project,
        3,
        json!({ "version": 3, "name": name }),
        &Agent::System,
        OffsetDateTime::UNIX_EPOCH,
    )
}

async fn heard(wired: &Wired, message: &Message) {
    wired
        .abandoning
        .handle(message)
        .await
        .expect("messages come at least once");
    wired.settle().await;
}

async fn is_deleted(wired: &Wired, id: &str) -> bool {
    matches!(
        wired
            .projects
            .rename(id, "Too late", None, &Agent::Local)
            .await,
        Err(ProjectsError::Events(ServiceError::Refused(
            ProjectError::Deleted
        )))
    )
}

#[tokio::test]
async fn a_project_nobody_is_left_in_is_deleted_and_unlisted_however_often_it_is_told() {
    // given: the listener
    let wired = bench();
    // expect: it hears members abandoned
    assert!(
        wired
            .abandoning
            .hears(&RoutingKey::parse(ABANDONED).expect("a declared key"))
    );
    // given: a project
    let id = a_project(&wired).await;

    // when: its members are heard abandoned, twice
    for _ in 0..2 {
        heard(&wired, &abandoned(&id, "ABANDONED")).await;
    }

    // then: it is deleted
    assert!(is_deleted(&wired, &id).await);
    // and: it is listed no more
    assert!(
        wired
            .projects
            .list(&Agent::Local)
            .await
            .expect("listing should succeed")
            .is_empty()
    );
}

#[tokio::test]
async fn a_project_nobody_is_left_in_is_deleted_out_of_the_trash_before_its_time() {
    // given: a project in the trash
    let wired = bench();
    let id = a_project(&wired).await;
    wired
        .projects
        .trash(&id, None, &Agent::Local)
        .await
        .expect("trashing should succeed");
    wired.settle().await;

    // when: its members are heard abandoned
    heard(&wired, &abandoned(&id, "ABANDONED")).await;

    // then: it is deleted
    assert!(is_deleted(&wired, &id).await);
    // and: it is no longer due in the trash, nor listed there
    assert!(
        wired
            .trash
            .due(OffsetDateTime::UNIX_EPOCH + Duration::days(36_500), 100)
            .await
            .expect("asking the trash should succeed")
            .is_empty(),
        "taken out of the trash with the deletion"
    );
    assert!(
        wired
            .projects
            .trash_list(&Agent::Local)
            .await
            .expect("listing the trash should succeed")
            .is_empty()
    );
}

#[tokio::test]
async fn what_tells_no_project_abandoned_is_refused() {
    // given: a project
    let wired = bench();
    let id = a_project(&wired).await;

    // expect: hearing what is no abandoning fails
    assert!(
        wired
            .abandoning
            .handle(&abandoned(&id, "DISBANDED"))
            .await
            .is_err(),
        "a message the listener cannot act on belongs in dead letters"
    );
    // and: the project is still found
    assert!(wired.projects.read(&id, &Agent::Local).await.is_ok());
}
