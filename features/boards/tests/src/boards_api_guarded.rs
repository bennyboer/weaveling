use std::sync::Arc;
use std::sync::atomic::Ordering;

use access::Role;
use axum::Extension;
use axum::http::StatusCode;
use axum_test::{TestResponse, TestServer};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use live::testing::Subscriber;
use serde_json::{Value, json};
use time::OffsetDateTime;

use crate::wiring;
use crate::wiring::{Faltering, Wired};

const PROJECT: &str = "project_1";

struct Guarded {
    wired: Wired,
    access: Arc<Faltering>,
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

impl Guarded {
    async fn give(&self, account: &str, role: Option<Role>) {
        self.wired
            .give(PROJECT, &AccountId::from(account), role)
            .await;
    }
}

async fn a_board_in_ada_s_project() -> Guarded {
    let access = Arc::new(Faltering::default());
    let wired = wiring::wired_keeping(
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        access.clone(),
        true,
    );
    wired
        .give(PROJECT, &AccountId::from("account_ada"), Some(Role::Owner))
        .await;
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
        access,
        wired,
        board,
    }
}

async fn ideas_on(guarded: &Guarded) -> Vec<Value> {
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
    // given: a board grace may not read
    let guarded = a_board_in_ada_s_project().await;

    // when: she opens, reads and changes it
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

    // then: each is not found
    for answered in [opened, read, pinning, reshaping, unpinning] {
        answered.assert_status(StatusCode::NOT_FOUND);
        assert_eq!(answered.json::<Value>()["type"], "not-found");
    }
    // and: following it ends as not there
    let mut hers = Subscriber::on(&guarded.grace).await;
    hers.subscribe("board", "boards", &[("board", &guarded.board)])
        .await;
    assert_eq!(hers.ended().await, "not-there");
    // and: the board is unchanged
    assert_eq!(ideas_on(&guarded).await, vec![json!("idea_1")]);
}

#[tokio::test]
async fn a_reader_opens_reads_and_follows_the_board_and_is_told_only_an_author_may_change_it() {
    // given: a board
    let guarded = a_board_in_ada_s_project().await;
    // and: grace a reader of its project
    guarded.give("account_grace", Some(Role::Reader)).await;

    // when: she opens it
    let opened = guarded
        .grace
        .post("/boards")
        .json(&json!({ "project": PROJECT }))
        .await;
    // then: she finds the board
    opened.assert_status_ok();
    assert_eq!(opened.json::<Value>()["id"], guarded.board.as_str());
    // and: she may read it
    guarded
        .grace
        .get(&format!("/boards/{}", guarded.board))
        .await
        .assert_status_ok();
    // and: she may follow it
    let mut hers = Subscriber::on(&guarded.grace).await;
    hers.subscribe("board", "boards", &[("board", &guarded.board)])
        .await;
    hers.frame().await;
    // and: each change she makes is refused as not by an author
    for refused in changes_by(&guarded, &guarded.grace).await {
        refused.assert_status(StatusCode::FORBIDDEN);
        assert_eq!(refused.json::<Value>()["type"], "not-an-author");
    }
    // and: the board is unchanged
    assert_eq!(ideas_on(&guarded).await, vec![json!("idea_1")]);
}

#[tokio::test]
async fn an_author_pins_moves_and_unpins() {
    // given: a board
    let guarded = a_board_in_ada_s_project().await;
    // and: grace an author of its project
    guarded.give("account_grace", Some(Role::Author)).await;

    // when: she pins, moves and unpins
    let [pinning, reshaping, unpinning] = changes_by(&guarded, &guarded.grace).await;

    // then: each is done
    pinning.assert_status_ok();
    reshaping.assert_status_ok();
    unpinning.assert_status(StatusCode::NO_CONTENT);
    // and: the board shows it
    assert_eq!(ideas_on(&guarded).await, vec![json!("idea_2")]);
}

#[tokio::test]
async fn whom_to_let_in_not_being_known_is_a_failure_not_a_no() {
    // given: a board
    let guarded = a_board_in_ada_s_project().await;
    // and: who may do what cannot be known
    guarded.access.down.store(true, Ordering::Release);

    // then: opening and reading it fail
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
    // and: following it ends as failed
    let mut ada_s = Subscriber::on(&guarded.ada).await;
    ada_s
        .subscribe("board", "boards", &[("board", &guarded.board)])
        .await;
    assert_eq!(ada_s.ended().await, "failed");
}

#[tokio::test]
async fn a_follower_whose_role_is_taken_away_is_closed_out() {
    // given: a board
    let guarded = a_board_in_ada_s_project().await;
    // and: grace following it as a reader
    guarded.give("account_grace", Some(Role::Reader)).await;
    let mut hers = Subscriber::on(&guarded.grace).await;
    hers.subscribe("board", "boards", &[("board", &guarded.board)])
        .await;
    hers.frame().await;

    // when: her role is taken away
    guarded.give("account_grace", None).await;

    // then: her following ends as access lost
    assert_eq!(hers.ended().await, "access-lost");
}
