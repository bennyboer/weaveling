use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use authorizing::{AccessChanges, AccessWatch, Act, Authorizer, AuthorizerError, Readable};
use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use live::testing::Subscriber;
use live_contract::{ACCESS_CHANGED, ToldDTO};
use serde_json::{Value, json};
use time::OffsetDateTime;

use crate::wiring;
use crate::wiring::Wired;

const PROJECT: &str = "project_1";
const OWNER: [Act; 3] = [Act::Read, Act::Write, Act::Manage];
const READER: [Act; 1] = [Act::Read];

#[derive(Default)]
struct Roles {
    held: Mutex<HashMap<String, Vec<Act>>>,
    down: Mutex<bool>,
    changes: AccessChanges,
}

impl Roles {
    fn give(&self, account: &str, acts: &[Act]) {
        self.held
            .lock()
            .expect("never poisoned")
            .insert(account.to_owned(), acts.to_vec());
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

        Ok(project == PROJECT
            && self
                .held
                .lock()
                .expect("never poisoned")
                .get(account.as_str())
                .is_some_and(|acts| acts.contains(&act)))
    }

    async fn readable_by(&self, _agent: &Agent) -> Result<Readable, AuthorizerError> {
        Ok(Readable::Everything)
    }

    fn access_changed(&self, project: &str) {
        self.changes.tell(project);
    }

    fn watch_access(&self, project: &str) -> AccessWatch {
        self.changes.watch(project)
    }
}

struct Guarded {
    wired: Wired,
    roles: Arc<Roles>,
    ada: TestServer,
    grace: TestServer,
    idea: String,
}

fn as_account(wired: &Wired, account: &str) -> TestServer {
    TestServer::builder().http_transport().build(
        wired
            .routes
            .clone()
            .layer(Extension(Agent::Account(AccountId::from(account)))),
    )
}

async fn guarded() -> Guarded {
    let roles = Arc::new(Roles::default());
    roles.give("account_ada", &OWNER);
    let wired = wiring::wired_guarded_by(
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        roles.clone(),
    );
    let ada = as_account(&wired, "account_ada");
    let captured = ada
        .post("/ideas")
        .json(&json!({ "project": PROJECT, "title": "A girl in a wood" }))
        .await;
    captured.assert_status(StatusCode::CREATED);
    let idea = captured.json::<Value>()["id"]
        .as_str()
        .expect("an idea has an id")
        .to_owned();
    wired.settle().await;

    Guarded {
        grace: as_account(&wired, "account_grace"),
        ada,
        roles,
        wired,
        idea,
    }
}

async fn titled(guarded: &Guarded) -> Value {
    guarded
        .ada
        .get(&format!("/ideas/{}", guarded.idea))
        .await
        .json::<Value>()["title"]
        .clone()
}

#[tokio::test]
async fn the_ideas_of_a_project_are_not_there_for_whoever_may_not_read_it() {
    let guarded = guarded().await;
    let at = format!("/ideas/{}", guarded.idea);

    for answered in [
        guarded
            .grace
            .get(&format!("/ideas?project={PROJECT}"))
            .await,
        guarded
            .grace
            .post("/ideas")
            .json(&json!({ "project": PROJECT, "title": "Theirs" }))
            .await,
        guarded.grace.get(&at).await,
        guarded
            .grace
            .patch(&at)
            .json(&json!({ "title": "Theirs" }))
            .await,
        guarded.grace.delete(&at).await,
    ] {
        answered.assert_status(StatusCode::NOT_FOUND);
        assert_eq!(answered.json::<Value>()["type"], "not-found");
    }
    let mut hers = Subscriber::on(&guarded.grace).await;
    hers.subscribe("ideas", "ideas", &[("project", PROJECT)])
        .await;
    assert_eq!(hers.ended().await, "not-there");
    assert_eq!(titled(&guarded).await, "A girl in a wood");
}

#[tokio::test]
async fn a_reader_reads_and_follows_the_ideas_and_is_told_only_an_author_may_change_them() {
    let guarded = guarded().await;
    guarded.roles.give("account_grace", &READER);
    let at = format!("/ideas/{}", guarded.idea);

    let listed = guarded
        .grace
        .get(&format!("/ideas?project={PROJECT}"))
        .await;
    listed.assert_status_ok();
    assert_eq!(listed.json::<Value>()[0]["id"], guarded.idea.as_str());
    guarded.grace.get(&at).await.assert_status_ok();
    let mut hers = Subscriber::on(&guarded.grace).await;
    hers.subscribe("ideas", "ideas", &[("project", PROJECT)])
        .await;
    hers.nothing().await;
    for refused in [
        guarded
            .grace
            .post("/ideas")
            .json(&json!({ "project": PROJECT, "title": "Theirs" }))
            .await,
        guarded
            .grace
            .patch(&at)
            .json(&json!({ "title": "Theirs" }))
            .await,
        guarded.grace.delete(&at).await,
    ] {
        refused.assert_status(StatusCode::FORBIDDEN);
        assert_eq!(refused.json::<Value>()["type"], "not-an-author");
    }
    assert_eq!(titled(&guarded).await, "A girl in a wood");
}

#[tokio::test]
async fn an_author_captures_retitles_and_discards() {
    let guarded = guarded().await;
    guarded
        .roles
        .give("account_grace", &[Act::Read, Act::Write]);
    let at = format!("/ideas/{}", guarded.idea);

    guarded
        .grace
        .post("/ideas")
        .json(&json!({ "project": PROJECT, "title": "Hers" }))
        .await
        .assert_status(StatusCode::CREATED);
    guarded
        .grace
        .patch(&at)
        .json(&json!({ "title": "A boy in a wood" }))
        .await
        .assert_status_ok();
    assert_eq!(titled(&guarded).await, "A boy in a wood");
    guarded
        .grace
        .delete(&at)
        .await
        .assert_status(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn a_follower_whose_role_is_taken_away_is_closed_out_and_the_others_follow_on() {
    let guarded = guarded().await;
    guarded.roles.give("account_grace", &READER);
    let mut hers = Subscriber::on(&guarded.grace).await;
    hers.subscribe("ideas", "ideas", &[("project", PROJECT)])
        .await;
    let mut ada_s = Subscriber::on(&guarded.ada).await;
    ada_s
        .subscribe("ideas", "ideas", &[("project", PROJECT)])
        .await;

    guarded.roles.give("account_grace", &[]);
    guarded.roles.access_changed(PROJECT);

    assert_eq!(
        hers.ended().await,
        "access-lost",
        "a member removed hears nothing more of the project"
    );
    assert!(
        matches!(ada_s.told().await, ToldDTO::Notice { notice, .. } if notice == ACCESS_CHANGED),
        "the owner stays and is told the members changed"
    );
    guarded
        .ada
        .post("/ideas")
        .json(&json!({ "project": PROJECT, "title": "Told to the owner" }))
        .await
        .assert_status(StatusCode::CREATED);
    guarded.wired.settle().await;
    assert_eq!(
        ada_s.frame().await["event"]["title"],
        "Told to the owner",
        "the owner still follows"
    );
}

#[tokio::test]
async fn whom_to_let_in_not_being_known_is_a_failure_not_a_no() {
    let guarded = guarded().await;
    *guarded.roles.down.lock().expect("never poisoned") = true;

    for answered in [
        guarded.ada.get(&format!("/ideas?project={PROJECT}")).await,
        guarded.ada.get(&format!("/ideas/{}", guarded.idea)).await,
    ] {
        answered.assert_status(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let mut ada_s = Subscriber::on(&guarded.ada).await;
    ada_s
        .subscribe("ideas", "ideas", &[("project", PROJECT)])
        .await;
    assert_eq!(ada_s.ended().await, "failed");
}
