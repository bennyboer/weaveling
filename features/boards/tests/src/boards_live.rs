use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use axum_test::{TestServer, TestWebSocket};
use boards_contract::{BoardDTO, OpenBoardRequest, PinIdeaRequest, SizeDTO, SpotDTO};
use clock::FixedClock;
use serde_json::Value;
use time::OffsetDateTime;
use tokio::time::timeout;

use crate::wiring::{Wired, wired};

fn served() -> (Wired, TestServer) {
    let wired = wired(Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)));
    let server = TestServer::builder()
        .http_transport()
        .build(wired.routes.clone());

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

async fn following(server: &TestServer, board: &str) -> TestWebSocket {
    server
        .get_websocket(&format!("/boards/live?board={board}"))
        .await
        .into_websocket()
        .await
}

async fn told(socket: &mut TestWebSocket) -> Value {
    let frame = timeout(Duration::from_secs(1), socket.receive_text())
        .await
        .expect("a change is told while it is fresh");

    serde_json::from_str(&frame).expect("a frame is JSON")
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
