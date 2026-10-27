use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use authorizing::{AccessChanges, AccessWatch, Act, Authorizer, AuthorizerError, Readable};
use axum::Extension;
use axum::http::StatusCode;
use axum_test::{TestResponse, TestServer, WsMessage};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use live_contract::NO_LONGER_ADMITTED;
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
    _wired: Wired,
    roles: Arc<Roles>,
    ada: TestServer,
    grace: TestServer,
    board: String,
}

fn as_account(wired: &Wired, account: &str) -> TestServer {
    TestServer::builder().http_transport().build(
        wired
            .routes
            .clone()
            .layer(Extension(Agent::Account(AccountId::from(account)))),
    )
}

fn a_card(idea: &str) -> Value {
    json!({ "idea": idea, "spot": { "x": 0, "y": 0 }, "size": { "width": 168, "height": 84 } })
}

async fn guarded() -> Guarded {
    let roles = Arc::new(Roles::default());
    roles.give("account_ada", &OWNER);
    let wired = wiring::wired_guarded_by(
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        roles.clone(),
    );
    let ada = as_account(&wired, "account_ada");
    let opened = ada
        .post("/boards")
        .json(&json!({ "project": PROJECT }))
        .await;
    opened.assert_status_ok();
    let board = opened.json::<Value>()["id"]
        .as_str()
        .expect("a board has an id")
        .to_owned();
    ada.post(&format!("/boards/{board}/ideas"))
        .json(&a_card("idea_1"))
        .await
        .assert_status_ok();
    wired.settle().await;

    Guarded {
        grace: as_account(&wired, "account_grace"),
        ada,
        roles,
        _wired: wired,
        board,
    }
}

async fn pinned(guarded: &Guarded) -> Vec<Value> {
    guarded
        .ada
        .get(&format!("/boards/{}", guarded.board))
        .await
        .json::<Value>()["ideas"]
        .as_array()
        .expect("a board lists its ideas")
        .iter()
        .map(|card| card["idea"].clone())
        .collect()
}

async fn changes_by(guarded: &Guarded, account: &TestServer) -> [TestResponse; 3] {
    let board = &guarded.board;

    [
        account
            .post(&format!("/boards/{board}/ideas"))
            .json(&a_card("idea_2"))
            .await,
        account
            .patch(&format!("/boards/{board}/ideas/idea_1"))
            .json(&json!({ "spot": { "x": 40, "y": 40 } }))
            .await,
        account
            .delete(&format!("/boards/{board}/ideas/idea_1"))
            .await,
    ]
}

#[tokio::test]
async fn a_project_s_board_is_not_there_for_whoever_may_not_read_it() {
    let guarded = guarded().await;

    let opened = guarded
        .grace
        .post("/boards")
        .json(&json!({ "project": PROJECT }))
        .await;
    let read = guarded
        .grace
        .get(&format!("/boards/{}", guarded.board))
        .await;
    let [pinning, reshaping, unpinning] = changes_by(&guarded, &guarded.grace).await;

    for answered in [opened, read, pinning, reshaping, unpinning] {
        answered.assert_status(StatusCode::NOT_FOUND);
        assert_eq!(answered.json::<Value>()["type"], "not-found");
    }
    guarded
        .grace
        .get_websocket(&format!("/boards/live?board={}", guarded.board))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    assert_eq!(pinned(&guarded).await, vec![json!("idea_1")]);
}

#[tokio::test]
async fn a_reader_opens_reads_and_follows_the_board_and_is_told_only_an_author_may_change_it() {
    let guarded = guarded().await;
    guarded.roles.give("account_grace", &READER);

    let opened = guarded
        .grace
        .post("/boards")
        .json(&json!({ "project": PROJECT }))
        .await;
    opened.assert_status_ok();
    assert_eq!(opened.json::<Value>()["id"], guarded.board.as_str());
    guarded
        .grace
        .get(&format!("/boards/{}", guarded.board))
        .await
        .assert_status_ok();
    guarded
        .grace
        .get_websocket(&format!("/boards/live?board={}", guarded.board))
        .await
        .assert_status(StatusCode::SWITCHING_PROTOCOLS);
    for refused in changes_by(&guarded, &guarded.grace).await {
        refused.assert_status(StatusCode::FORBIDDEN);
        assert_eq!(refused.json::<Value>()["type"], "not-an-author");
    }
    assert_eq!(pinned(&guarded).await, vec![json!("idea_1")]);
}

#[tokio::test]
async fn an_author_pins_moves_and_unpins() {
    let guarded = guarded().await;
    guarded
        .roles
        .give("account_grace", &[Act::Read, Act::Write]);

    let [pinning, reshaping, unpinning] = changes_by(&guarded, &guarded.grace).await;

    pinning.assert_status_ok();
    reshaping.assert_status_ok();
    unpinning.assert_status(StatusCode::NO_CONTENT);
    assert_eq!(pinned(&guarded).await, vec![json!("idea_2")]);
}

#[tokio::test]
async fn whom_to_let_in_not_being_known_is_a_failure_not_a_no() {
    let guarded = guarded().await;
    *guarded.roles.down.lock().expect("never poisoned") = true;

    for answered in [
        guarded
            .ada
            .post("/boards")
            .json(&json!({ "project": PROJECT }))
            .await,
        guarded.ada.get(&format!("/boards/{}", guarded.board)).await,
    ] {
        answered.assert_status(StatusCode::INTERNAL_SERVER_ERROR);
    }
}

#[tokio::test]
async fn a_follower_whose_role_is_taken_away_is_closed_out() {
    let guarded = guarded().await;
    guarded.roles.give("account_grace", &READER);
    let mut hers = guarded
        .grace
        .get_websocket(&format!("/boards/live?board={}", guarded.board))
        .await
        .into_websocket()
        .await;
    _ = hers.receive_text().await;

    guarded.roles.give("account_grace", &[]);
    guarded.roles.access_changed(PROJECT);

    assert!(matches!(
        tokio::time::timeout(std::time::Duration::from_secs(1), hers.receive_message()).await,
        Ok(WsMessage::Close(Some(frame))) if u16::from(frame.code) == NO_LONGER_ADMITTED
    ));
}
