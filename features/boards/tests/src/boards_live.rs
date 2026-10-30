use std::sync::Arc;

use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use boards_contract::{BoardDTO, OpenBoardRequest, PinIdeaRequest, SizeDTO, SpotDTO};
use clock::FixedClock;
use eventsourcing::Agent;
use live::testing::Subscriber;
use serde_json::{Value, json};
use time::OffsetDateTime;

use crate::wiring;
use crate::wiring::Wired;

fn served() -> (Wired, TestServer) {
    let wired = wiring::wired(Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)));
    let server = TestServer::builder()
        .http_transport()
        .build(wired.routes.clone().layer(Extension(Agent::Local)));

    (wired, server)
}

async fn an_open_board(server: &TestServer, project: &str) -> String {
    let response = server
        .post("/boards")
        .json(&OpenBoardRequest {
            project: project.to_owned(),
        })
        .await;
    response.assert_status(StatusCode::OK);

    response.json::<BoardDTO>().id
}

async fn an_idea_pinned(server: &TestServer, board: &str, idea: &str) {
    server
        .post(&format!("/boards/{board}/ideas"))
        .json(&PinIdeaRequest {
            idea: idea.to_owned(),
            spot: SpotDTO { x: 0, y: 0 },
            size: SizeDTO {
                width: 168,
                height: 84,
            },
        })
        .await
        .assert_status_ok();
}

async fn connected(server: &TestServer, board: &str) -> Subscriber {
    let mut socket = Subscriber::on(server).await;
    socket
        .subscribe("board", "boards", &[("board", board)])
        .await;

    socket
}

async fn following(server: &TestServer, board: &str) -> Subscriber {
    let mut socket = connected(server, board).await;
    socket.frame().await;

    socket
}

async fn told(socket: &mut Subscriber) -> Value {
    socket.frame().await
}

#[tokio::test]
async fn whoever_follows_a_board_is_told_each_change_by_its_version() {
    let (wired, server) = served();
    let board = an_open_board(&server, "project_1").await;
    wired.settle().await;
    let mut socket = following(&server, &board).await;

    an_idea_pinned(&server, &board, "idea_1").await;
    wired.settle().await;

    let frame = told(&mut socket).await;
    let now: BoardDTO = server.get(&format!("/boards/{board}")).await.json();
    assert_eq!(frame["version"], now.version);
    assert_eq!(frame["event"]["name"], "IDEA_PINNED");
}

#[tokio::test]
async fn another_board_changing_is_not_told() {
    let (wired, server) = served();
    let mine = an_open_board(&server, "project_1").await;
    let theirs = an_open_board(&server, "project_2").await;
    wired.settle().await;
    let mut socket = following(&server, &mine).await;

    an_idea_pinned(&server, &theirs, "idea_theirs").await;
    an_idea_pinned(&server, &mine, "idea_mine").await;
    wired.settle().await;

    let frame = told(&mut socket).await;
    assert_eq!(
        frame["event"]["idea"], "idea_mine",
        "the first thing told is about the board being followed"
    );
}

#[tokio::test]
async fn whoever_follows_a_board_is_first_told_the_version_it_stands_at() {
    let (wired, server) = served();
    let board = an_open_board(&server, "project_1").await;
    wired.settle().await;
    let now: BoardDTO = server.get(&format!("/boards/{board}")).await.json();

    let mut socket = connected(&server, &board).await;

    assert_eq!(
        told(&mut socket).await,
        json!({ "version": now.version }),
        "a follower holding this version has missed nothing, one holding less opens it again"
    );
}

async fn on_one_socket(wired: &Wired) -> (TestServer, Subscriber) {
    let live = TestServer::builder().http_transport().build(
        live::live_socket(live::Topics::of(wired.topics.clone())).layer(Extension(Agent::Local)),
    );
    let socket = Subscriber::on(&live).await;

    (live, socket)
}

async fn told_on_one_socket(socket: &mut Subscriber) -> Value {
    serde_json::to_value(socket.told().await).expect("what is told is plain data")
}

#[tokio::test]
async fn on_the_one_socket_a_board_is_told_where_it_stands_then_each_change() {
    let (wired, server) = served();
    let board = an_open_board(&server, "project_1").await;
    wired.settle().await;
    let (_live, mut socket) = on_one_socket(&wired).await;

    socket
        .subscribe("board", "boards", &[("board", &board)])
        .await;
    let standing = told_on_one_socket(&mut socket).await;
    an_idea_pinned(&server, &board, "idea_1").await;
    wired.settle().await;
    let pinned = told_on_one_socket(&mut socket).await;
    socket
        .subscribe("nowhere", "boards", &[("board", "board_nobody_opened")])
        .await;
    let nowhere = told_on_one_socket(&mut socket).await;

    assert_eq!(standing["id"], "board");
    assert_eq!(
        standing["frame"],
        json!({ "version": 1 }),
        "told first where it stands"
    );
    assert_eq!(pinned["frame"]["of"], board);
    assert_eq!(pinned["frame"]["version"], 2);
    assert_eq!(
        nowhere,
        json!({ "told": "ended", "id": "nowhere", "why": "not-there" })
    );
}
