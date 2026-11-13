use std::sync::Arc;

use access::Role;
use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use serde_json::{Value, json};
use time::OffsetDateTime;

use crate::wiring;
use crate::wiring::Wired;

struct Guarded {
    wired: Wired,
    ada: TestServer,
    grace: TestServer,
}

fn as_account(wired: &Wired, account: &str) -> TestServer {
    TestServer::new(
        wired
            .routes
            .clone()
            .layer(Extension(Agent::Account(AccountId::from(account)))),
    )
}

async fn a_project_ada_owns() -> (Guarded, String) {
    let wired = wiring::wired_with_accounts(Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)));
    let guarded = Guarded {
        ada: as_account(&wired, "account_ada"),
        grace: as_account(&wired, "account_grace"),
        wired,
    };
    let started = guarded
        .ada
        .post("/projects")
        .json(&json!({ "name": "The Loom" }))
        .await;
    started.assert_status(StatusCode::CREATED);
    let project = started.json::<Value>()["id"]
        .as_str()
        .expect("a project has an id")
        .to_owned();
    guarded
        .wired
        .give(&project, "account_ada", Some(Role::Owner))
        .await;

    (guarded, project)
}

async fn name_of(server: &TestServer, project: &str) -> Value {
    server
        .get(&format!("/projects/{project}"))
        .await
        .json::<Value>()["name"]
        .clone()
}

#[tokio::test]
async fn a_project_is_not_there_for_whoever_may_not_read_it_whatever_they_try() {
    // given: a project ada owns and grace may not read
    let (guarded, project) = a_project_ada_owns().await;
    let at = format!("/projects/{project}");

    // when: grace reads, renames, describes and deletes it
    let read = guarded.grace.get(&at).await;
    let renamed = guarded
        .grace
        .patch(&at)
        .json(&json!({ "name": "Theirs" }))
        .await;
    let described = guarded
        .grace
        .put(&format!("{at}/details"))
        .json(&json!({ "language": null, "description": "theirs" }))
        .await;
    let deleted = guarded.grace.delete(&at).await;

    // then: each is answered as not found
    for answered in [&read, &renamed, &described, &deleted] {
        answered.assert_status(StatusCode::NOT_FOUND);
    }
    // and: the same way a project that is not there is
    let missing = guarded
        .ada
        .get("/projects/project_0000000000000000000000")
        .await;
    missing.assert_status(StatusCode::NOT_FOUND);
    assert_eq!(
        read.json::<Value>()["type"],
        missing.json::<Value>()["type"],
        "a project kept from someone answers as one that is not there, so its being is not told"
    );
    // and: the project is unchanged
    assert_eq!(name_of(&guarded.ada, &project).await, "The Loom");
}

#[tokio::test]
async fn whoever_may_read_but_not_manage_reads_and_is_told_only_an_owner_may_change_it() {
    // given: a project ada owns
    let (guarded, project) = a_project_ada_owns().await;
    // and: grace an author of it
    guarded
        .wired
        .give(&project, "account_grace", Some(Role::Author))
        .await;
    let at = format!("/projects/{project}");

    // expect: grace reads it
    guarded.grace.get(&at).await.assert_status_ok();
    // and: grace is told only an owner may rename, describe or delete it
    for refused in [
        guarded
            .grace
            .patch(&at)
            .json(&json!({ "name": "Theirs" }))
            .await,
        guarded
            .grace
            .put(&format!("{at}/details"))
            .json(&json!({ "language": null, "description": "theirs" }))
            .await,
        guarded.grace.delete(&at).await,
    ] {
        refused.assert_status(StatusCode::FORBIDDEN);
        assert_eq!(refused.json::<Value>()["type"], "not-an-owner");
    }
    // and: the project is unchanged
    assert_eq!(name_of(&guarded.ada, &project).await, "The Loom");
}

#[tokio::test]
async fn whoever_may_only_read_reads_the_project() {
    // given: a project ada owns
    let (guarded, project) = a_project_ada_owns().await;
    // and: grace a reader of it
    guarded
        .wired
        .give(&project, "account_grace", Some(Role::Reader))
        .await;

    // expect: grace reads it
    guarded
        .grace
        .get(&format!("/projects/{project}"))
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn an_owner_reads_renames_describes_and_deletes() {
    // given: a project ada owns
    let (guarded, project) = a_project_ada_owns().await;
    let at = format!("/projects/{project}");

    // expect: ada renames it
    guarded
        .ada
        .patch(&at)
        .json(&json!({ "name": "The Heddle" }))
        .await
        .assert_status_ok();
    // and: ada describes it
    guarded
        .ada
        .put(&format!("{at}/details"))
        .json(&json!({ "language": "de", "description": "Ein Webstuhl" }))
        .await
        .assert_status_ok();
    // and: ada reads it renamed
    assert_eq!(name_of(&guarded.ada, &project).await, "The Heddle");
    // and: ada deletes it
    guarded
        .ada
        .delete(&at)
        .await
        .assert_status(StatusCode::NO_CONTENT);
    // and: it is gone
    guarded
        .ada
        .get(&at)
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn whom_to_let_in_not_being_known_is_a_failure_not_a_no() {
    // given: a project ada owns
    let (guarded, project) = a_project_ada_owns().await;
    // and: who may do what cannot be learned
    guarded.wired.access.go_down();

    // expect: reading it fails
    guarded
        .ada
        .get(&format!("/projects/{project}"))
        .await
        .assert_status(StatusCode::INTERNAL_SERVER_ERROR);
}
