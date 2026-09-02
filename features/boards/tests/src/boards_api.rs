use std::sync::Arc;

use axum::http::StatusCode;
use axum_test::TestServer;
use boards_contract::{
    BoardDTO, MovePieceRequest, OpenBoardRequest, PinPieceRequest, PositionedPieceDTO, SpotDTO,
};
use clock::FixedClock;
use time::{Duration, OffsetDateTime};

use crate::wiring::wired;

const UNKNOWN_BOARD: &str = "board_031VkO0hnpeQZUiAB7nDma";

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn a_server() -> TestServer {
    TestServer::new(wired(Arc::new(FixedClock::new(at(1_700_000_000)))).routes)
}

fn opening(project: &str) -> OpenBoardRequest {
    OpenBoardRequest {
        project: project.to_owned(),
    }
}

fn a_spot(x: i64, y: i64) -> SpotDTO {
    SpotDTO { x, y }
}

async fn an_open_board(server: &TestServer) -> BoardDTO {
    let response = server.post("/boards").json(&opening("project_1")).await;
    response.assert_status(StatusCode::OK);

    response.json()
}

async fn a_pinned_piece(
    server: &TestServer,
    board: &BoardDTO,
    piece: &str,
    at: SpotDTO,
) -> BoardDTO {
    let response = server
        .post(&format!("/boards/{}/pieces", board.id))
        .json(&PinPieceRequest {
            piece: piece.to_owned(),
            spot: at,
        })
        .await;
    response.assert_status(StatusCode::OK);

    response.json()
}

#[tokio::test]
async fn opening_a_board_comes_back_with_a_prefixed_id() {
    let server = a_server();

    let opened = an_open_board(&server).await;

    assert!(
        opened.id.starts_with("board_"),
        "an id should say what it is: {}",
        opened.id
    );
    assert_eq!(opened.project, "project_1");
    assert_eq!(opened.version, 1);
    assert!(opened.pieces.is_empty());
}

#[tokio::test]
async fn opening_a_board_twice_returns_the_same_board() {
    let server = a_server();
    let first = an_open_board(&server).await;

    let second = an_open_board(&server).await;

    assert_eq!(
        first.id, second.id,
        "posting to open is find-or-start, so it is safe to repeat"
    );
    assert_eq!(second.version, 1, "nothing further happened to it");
}

#[tokio::test]
async fn a_board_carries_an_etag_of_its_version() {
    let server = a_server();

    let response = server.post("/boards").json(&opening("project_1")).await;

    assert_eq!(response.header("etag"), "\"1\"");
}

#[tokio::test]
async fn a_board_can_be_read_back_by_its_id() {
    let server = a_server();
    let opened = an_open_board(&server).await;

    let response = server.get(&format!("/boards/{}", opened.id)).await;
    response.assert_status(StatusCode::OK);

    assert_eq!(response.json::<BoardDTO>(), opened);
}

#[tokio::test]
async fn a_board_nobody_opened_is_not_found() {
    let server = a_server();

    server
        .get(&format!("/boards/{UNKNOWN_BOARD}"))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_id_of_another_kind_is_a_bad_request() {
    let server = a_server();

    server
        .get("/boards/piece_031VkO0hnpeQZUiAB7nDma")
        .await
        .assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_pinned_piece_comes_back_where_it_was_dropped() {
    let server = a_server();
    let opened = an_open_board(&server).await;

    let pinned = a_pinned_piece(&server, &opened, "piece_1", a_spot(120, -40)).await;

    assert_eq!(
        pinned.pieces,
        vec![PositionedPieceDTO {
            piece: "piece_1".to_owned(),
            spot: a_spot(120, -40),
        }]
    );
    assert_eq!(pinned.version, 2);
}

#[tokio::test]
async fn pinning_the_same_piece_twice_is_a_conflict() {
    let server = a_server();
    let opened = an_open_board(&server).await;
    a_pinned_piece(&server, &opened, "piece_1", a_spot(0, 0)).await;

    server
        .post(&format!("/boards/{}/pieces", opened.id))
        .json(&PinPieceRequest {
            piece: "piece_1".to_owned(),
            spot: a_spot(9, 9),
        })
        .await
        .assert_status(StatusCode::CONFLICT);
}

#[tokio::test]
async fn a_moved_piece_comes_back_at_its_new_spot() {
    let server = a_server();
    let opened = an_open_board(&server).await;
    a_pinned_piece(&server, &opened, "piece_1", a_spot(10, 10)).await;

    let response = server
        .patch(&format!("/boards/{}/pieces/piece_1", opened.id))
        .json(&MovePieceRequest {
            spot: a_spot(300, 20),
        })
        .await;
    response.assert_status(StatusCode::OK);

    assert_eq!(
        response.json::<BoardDTO>().pieces,
        vec![PositionedPieceDTO {
            piece: "piece_1".to_owned(),
            spot: a_spot(300, 20),
        }]
    );
}

#[tokio::test]
async fn moving_a_piece_that_is_not_on_the_board_is_not_found() {
    let server = a_server();
    let opened = an_open_board(&server).await;

    server
        .patch(&format!("/boards/{}/pieces/piece_1", opened.id))
        .json(&MovePieceRequest { spot: a_spot(1, 1) })
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unpinning_a_piece_that_is_not_on_the_board_is_not_found() {
    let server = a_server();
    let opened = an_open_board(&server).await;

    server
        .delete(&format!("/boards/{}/pieces/piece_1", opened.id))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_unpinned_piece_leaves_the_board() {
    let server = a_server();
    let opened = an_open_board(&server).await;
    a_pinned_piece(&server, &opened, "piece_1", a_spot(10, 10)).await;

    server
        .delete(&format!("/boards/{}/pieces/piece_1", opened.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    let response = server.get(&format!("/boards/{}", opened.id)).await;
    assert!(response.json::<BoardDTO>().pieces.is_empty());
}

#[tokio::test]
async fn a_move_against_a_stale_version_is_refused() {
    let server = a_server();
    let opened = an_open_board(&server).await;
    a_pinned_piece(&server, &opened, "piece_1", a_spot(10, 10)).await;

    server
        .patch(&format!("/boards/{}/pieces/piece_1", opened.id))
        .add_header("if-match", "\"1\"")
        .json(&MovePieceRequest {
            spot: a_spot(20, 20),
        })
        .await
        .assert_status(StatusCode::PRECONDITION_FAILED);
}

#[tokio::test]
async fn a_move_against_the_current_version_is_allowed() {
    let server = a_server();
    let opened = an_open_board(&server).await;
    let pinned = a_pinned_piece(&server, &opened, "piece_1", a_spot(10, 10)).await;

    server
        .patch(&format!("/boards/{}/pieces/piece_1", opened.id))
        .add_header("if-match", format!("\"{}\"", pinned.version))
        .json(&MovePieceRequest {
            spot: a_spot(20, 20),
        })
        .await
        .assert_status(StatusCode::OK);
}

#[tokio::test]
async fn an_if_match_that_is_not_a_version_is_a_bad_request() {
    let server = a_server();
    let opened = an_open_board(&server).await;

    server
        .delete(&format!("/boards/{}/pieces/piece_1", opened.id))
        .add_header("if-match", "\"not-a-version\"")
        .await
        .assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn boards_of_other_projects_are_left_alone() {
    let server = a_server();
    let mine = an_open_board(&server).await;

    let response = server.post("/boards").json(&opening("project_2")).await;
    response.assert_status(StatusCode::OK);

    assert_ne!(response.json::<BoardDTO>().id, mine.id);
}
