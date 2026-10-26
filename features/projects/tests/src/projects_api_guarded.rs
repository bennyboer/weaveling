use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use authorizing::{Act, Authorizer, AuthorizerError, Readable};
use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use serde_json::{Value, json};
use time::OffsetDateTime;

use crate::wiring;
use crate::wiring::Wired;

#[derive(Default)]
struct Roles {
    held: Mutex<HashMap<(String, String), Vec<Act>>>,
    down: Mutex<bool>,
}

impl Roles {
    fn give(&self, account: &str, project: &str, acts: &[Act]) {
        self.held
            .lock()
            .expect("never poisoned")
            .insert((account.to_owned(), project.to_owned()), acts.to_vec());
    }
}

#[async_trait]
impl Authorizer for Roles {
    async fn may(&self, agent: &Agent, act: Act, project: &str) -> Result<bool, AuthorizerError> {
        if *self.down.lock().expect("never poisoned") {
            return Err(AuthorizerError::Unreachable("the list is down".into()));
        }
        let Agent::Account(account) = agent else {
            return Ok(false);
        };

        Ok(self
            .held
            .lock()
            .expect("never poisoned")
            .get(&(account.to_string(), project.to_owned()))
            .is_some_and(|acts| acts.contains(&act)))
    }

    async fn readable_by(&self, _agent: &Agent) -> Result<Readable, AuthorizerError> {
        Ok(Readable::Everything)
    }
}

const OWNER: [Act; 3] = [Act::Read, Act::Write, Act::Manage];
const AUTHOR: [Act; 2] = [Act::Read, Act::Write];
const READER: [Act; 1] = [Act::Read];

struct Guarded {
    _wired: Wired,
    roles: Arc<Roles>,
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

async fn guarded() -> (Guarded, String) {
    let roles = Arc::new(Roles::default());
    let wired = wiring::wired_guarded_by(
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        roles.clone(),
    );
    let guarded = Guarded {
        ada: as_account(&wired, "account_ada"),
        grace: as_account(&wired, "account_grace"),
        roles,
        _wired: wired,
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
    guarded.roles.give("account_ada", &project, &OWNER);

    (guarded, project)
}

async fn named(server: &TestServer, project: &str) -> Value {
    server
        .get(&format!("/projects/{project}"))
        .await
        .json::<Value>()["name"]
        .clone()
}

#[tokio::test]
async fn a_project_is_not_there_for_whoever_may_not_read_it_whatever_they_try() {
    let (guarded, project) = guarded().await;
    let at = format!("/projects/{project}");

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

    for answered in [&read, &renamed, &described, &deleted] {
        answered.assert_status(StatusCode::NOT_FOUND);
    }
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
    assert_eq!(named(&guarded.ada, &project).await, "The Loom");
}

#[tokio::test]
async fn whoever_may_read_but_not_manage_reads_and_is_told_only_an_owner_may_change_it() {
    let (guarded, project) = guarded().await;
    guarded.roles.give("account_grace", &project, &AUTHOR);
    let at = format!("/projects/{project}");

    guarded.grace.get(&at).await.assert_status_ok();
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
    assert_eq!(named(&guarded.ada, &project).await, "The Loom");
}

#[tokio::test]
async fn whoever_may_only_read_reads_the_project() {
    let (guarded, project) = guarded().await;
    guarded.roles.give("account_grace", &project, &READER);

    guarded
        .grace
        .get(&format!("/projects/{project}"))
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn an_owner_reads_renames_describes_and_deletes() {
    let (guarded, project) = guarded().await;
    let at = format!("/projects/{project}");

    guarded
        .ada
        .patch(&at)
        .json(&json!({ "name": "The Heddle" }))
        .await
        .assert_status_ok();
    guarded
        .ada
        .put(&format!("{at}/details"))
        .json(&json!({ "language": "de", "description": "Ein Webstuhl" }))
        .await
        .assert_status_ok();
    assert_eq!(named(&guarded.ada, &project).await, "The Heddle");
    guarded
        .ada
        .delete(&at)
        .await
        .assert_status(StatusCode::NO_CONTENT);
    guarded
        .ada
        .get(&at)
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn whom_to_let_in_not_being_known_is_a_failure_not_a_no() {
    let (guarded, project) = guarded().await;
    *guarded.roles.down.lock().expect("never poisoned") = true;

    guarded
        .ada
        .get(&format!("/projects/{project}"))
        .await
        .assert_status(StatusCode::INTERNAL_SERVER_ERROR);
}
