use std::sync::Arc;

use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use pieces_contract::{AttachPassageRequest, CapturePieceRequest, PieceDTO, RetitlePieceRequest};
use pieces_core::PieceTitle;

use crate::wiring::wired;
use time::{Duration, OffsetDateTime};

const UNKNOWN_ID: &str = "piece_031VkO0hnpeQZUiAB7nDma";

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn a_server() -> TestServer {
    let wired = wired(Arc::new(FixedClock::new(at(1_700_000_000))));

    TestServer::new(wired.routes)
}

fn a_capture(title: &str) -> CapturePieceRequest {
    CapturePieceRequest {
        project: "project_1".to_owned(),
        title: title.to_owned(),
    }
}

async fn a_piece(server: &TestServer, title: &str) -> PieceDTO {
    let response = server.post("/pieces").json(&a_capture(title)).await;
    response.assert_status(StatusCode::CREATED);

    response.json()
}

#[tokio::test]
async fn a_captured_piece_comes_back_with_a_prefixed_id() {
    let server = a_server();

    let captured = a_piece(&server, "The Loom").await;

    assert!(
        captured.id.starts_with("piece_"),
        "an id should say what it is: {}",
        captured.id
    );
    assert_eq!(captured.title, "The Loom");
    assert_eq!(captured.project, "project_1");
    assert_eq!(
        captured.passage, None,
        "a passage is attached on first open"
    );
}

#[tokio::test]
async fn a_piece_may_be_captured_with_no_title() {
    let server = a_server();

    let captured = a_piece(&server, "").await;

    assert_eq!(captured.title, "");
}

#[tokio::test]
async fn two_captured_pieces_are_distinct() {
    let server = a_server();

    let one = a_piece(&server, "The Loom").await;
    let other = a_piece(&server, "The Loom").await;

    assert_ne!(one.id, other.id);
}

#[tokio::test]
async fn a_captured_piece_can_be_fetched_again() {
    let server = a_server();
    let captured = a_piece(&server, "The Loom").await;

    let response = server.get(&format!("/pieces/{}", captured.id)).await;

    response.assert_status_ok();
    assert_eq!(response.json::<PieceDTO>(), captured);
}

#[tokio::test]
async fn a_piece_nobody_captured_is_not_found() {
    let server = a_server();

    server
        .get(&format!("/pieces/{UNKNOWN_ID}"))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_id_of_another_kind_is_a_bad_request() {
    let server = a_server();

    server
        .get("/pieces/passage_031VkO0hnpeQZUiAB7nDma")
        .await
        .assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn an_id_without_its_prefix_is_a_bad_request() {
    let server = a_server();

    server
        .get("/pieces/031VkO0hnpeQZUiAB7nDma")
        .await
        .assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_piece_can_be_retitled() {
    let server = a_server();
    let captured = a_piece(&server, "The Loom").await;

    let response = server
        .patch(&format!("/pieces/{}", captured.id))
        .json(&RetitlePieceRequest {
            title: "The Silent Loom".to_owned(),
        })
        .await;

    response.assert_status_ok();
    assert_eq!(response.json::<PieceDTO>().title, "The Silent Loom");
}

#[tokio::test]
async fn a_title_the_domain_refuses_is_a_bad_request() {
    let server = a_server();
    let captured = a_piece(&server, "The Loom").await;

    server
        .patch(&format!("/pieces/{}", captured.id))
        .json(&RetitlePieceRequest {
            title: "a".repeat(PieceTitle::MAX_CHARS + 1),
        })
        .await
        .assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_passage_can_be_attached() {
    let server = a_server();
    let captured = a_piece(&server, "The Loom").await;

    let response = server
        .put(&format!("/pieces/{}/passage", captured.id))
        .json(&AttachPassageRequest {
            passage: "passage_9".to_owned(),
        })
        .await;

    response.assert_status_ok();
    assert_eq!(
        response.json::<PieceDTO>().passage,
        Some("passage_9".to_owned())
    );
}

#[tokio::test]
async fn a_second_passage_is_a_conflict() {
    let server = a_server();
    let captured = a_piece(&server, "The Loom").await;
    server
        .put(&format!("/pieces/{}/passage", captured.id))
        .json(&AttachPassageRequest {
            passage: "passage_9".to_owned(),
        })
        .await
        .assert_status_ok();

    server
        .put(&format!("/pieces/{}/passage", captured.id))
        .json(&AttachPassageRequest {
            passage: "passage_10".to_owned(),
        })
        .await
        .assert_status(StatusCode::CONFLICT);
}

#[tokio::test]
async fn a_discarded_piece_is_gone_from_the_authors_point_of_view() {
    let server = a_server();
    let captured = a_piece(&server, "The Loom").await;

    server
        .delete(&format!("/pieces/{}", captured.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    server
        .patch(&format!("/pieces/{}", captured.id))
        .json(&RetitlePieceRequest {
            title: "Too late".to_owned(),
        })
        .await
        .assert_status(StatusCode::CONFLICT);
}

#[tokio::test]
async fn discarding_something_that_was_never_captured_is_not_found() {
    let server = a_server();

    server
        .delete(&format!("/pieces/{UNKNOWN_ID}"))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_piece_reports_the_version_it_stands_at() {
    let server = a_server();

    let captured = a_piece(&server, "The Loom").await;

    assert_eq!(
        captured.version, 1,
        "a captured piece stands at its first version"
    );
}

#[tokio::test]
async fn every_response_carries_the_version_as_an_etag() {
    let server = a_server();
    let captured = a_piece(&server, "The Loom").await;

    let response = server.get(&format!("/pieces/{}", captured.id)).await;

    assert_eq!(
        response.header("etag").to_str().expect("an ascii etag"),
        "\"1\"",
        "a caller needs somewhere to read the version it is looking at"
    );
}

#[tokio::test]
async fn a_change_moves_the_version_on() {
    let server = a_server();
    let captured = a_piece(&server, "The Loom").await;

    let response = server
        .patch(&format!("/pieces/{}", captured.id))
        .json(&RetitlePieceRequest {
            title: "The Silent Loom".to_owned(),
        })
        .await;

    assert_eq!(response.json::<PieceDTO>().version, 2);
}

#[tokio::test]
async fn a_change_from_the_version_the_caller_saw_is_accepted() {
    let server = a_server();
    let captured = a_piece(&server, "The Loom").await;

    server
        .patch(&format!("/pieces/{}", captured.id))
        .add_header("if-match", "\"1\"")
        .json(&RetitlePieceRequest {
            title: "The Silent Loom".to_owned(),
        })
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn a_change_from_a_version_that_has_moved_on_is_refused() {
    let server = a_server();
    let captured = a_piece(&server, "The Loom").await;
    server
        .patch(&format!("/pieces/{}", captured.id))
        .json(&RetitlePieceRequest {
            title: "Someone else got here first".to_owned(),
        })
        .await
        .assert_status_ok();

    server
        .patch(&format!("/pieces/{}", captured.id))
        .add_header("if-match", "\"1\"")
        .json(&RetitlePieceRequest {
            title: "Working from a stale copy".to_owned(),
        })
        .await
        .assert_status(StatusCode::PRECONDITION_FAILED);

    let response = server.get(&format!("/pieces/{}", captured.id)).await;
    assert_eq!(
        response.json::<PieceDTO>().title,
        "Someone else got here first",
        "a refused change must leave the piece as it was"
    );
}

#[tokio::test]
async fn discarding_from_a_stale_version_is_refused() {
    let server = a_server();
    let captured = a_piece(&server, "The Loom").await;
    server
        .patch(&format!("/pieces/{}", captured.id))
        .json(&RetitlePieceRequest {
            title: "The Silent Loom".to_owned(),
        })
        .await
        .assert_status_ok();

    server
        .delete(&format!("/pieces/{}", captured.id))
        .add_header("if-match", "\"1\"")
        .await
        .assert_status(StatusCode::PRECONDITION_FAILED);

    server
        .get(&format!("/pieces/{}", captured.id))
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn any_version_will_do_when_the_caller_says_so() {
    let server = a_server();
    let captured = a_piece(&server, "The Loom").await;
    server
        .patch(&format!("/pieces/{}", captured.id))
        .json(&RetitlePieceRequest {
            title: "Moved on".to_owned(),
        })
        .await
        .assert_status_ok();

    server
        .patch(&format!("/pieces/{}", captured.id))
        .add_header("if-match", "*")
        .json(&RetitlePieceRequest {
            title: "Whatever it says now".to_owned(),
        })
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn an_if_match_that_is_not_a_version_is_a_bad_request() {
    let server = a_server();
    let captured = a_piece(&server, "The Loom").await;

    server
        .patch(&format!("/pieces/{}", captured.id))
        .add_header("if-match", "\"not-a-version\"")
        .json(&RetitlePieceRequest {
            title: "Nowhere".to_owned(),
        })
        .await
        .assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_version_that_never_existed_is_a_failed_precondition_not_a_missing_piece() {
    let server = a_server();
    let captured = a_piece(&server, "The Loom").await;

    server
        .patch(&format!("/pieces/{}", captured.id))
        .add_header("if-match", "\"0\"")
        .json(&RetitlePieceRequest {
            title: "Stale".to_owned(),
        })
        .await
        .assert_status(StatusCode::PRECONDITION_FAILED);

    server
        .patch(&format!("/pieces/{}", captured.id))
        .add_header("if-match", "\"99\"")
        .json(&RetitlePieceRequest {
            title: "From the future".to_owned(),
        })
        .await
        .assert_status(StatusCode::PRECONDITION_FAILED);
}

#[tokio::test]
async fn a_precondition_on_a_piece_nobody_captured_is_still_not_found() {
    let server = a_server();

    server
        .patch(&format!("/pieces/{UNKNOWN_ID}"))
        .add_header("if-match", "\"1\"")
        .json(&RetitlePieceRequest {
            title: "Nowhere".to_owned(),
        })
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_project_nobody_wrote_in_lists_nothing() {
    let server = a_server();

    let response = server.get("/pieces?project=project_empty").await;

    response.assert_status_ok();
    assert!(response.json::<Vec<PieceDTO>>().is_empty());
}

#[tokio::test]
async fn captured_pieces_are_listed_for_their_project() {
    let server = a_server();
    let one = a_piece(&server, "The Loom").await;
    let other = a_piece(&server, "The Shuttle").await;

    let listed = server
        .get("/pieces?project=project_1")
        .await
        .json::<Vec<PieceDTO>>();

    assert_eq!(listed.len(), 2);
    assert!(listed.iter().any(|piece| piece.id == one.id));
    assert!(listed.iter().any(|piece| piece.id == other.id));
}

#[tokio::test]
async fn listing_asks_for_a_project() {
    let server = a_server();

    server
        .get("/pieces")
        .await
        .assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_retitled_piece_is_listed_under_its_new_title() {
    let server = a_server();
    let captured = a_piece(&server, "The Loom").await;

    server
        .patch(&format!("/pieces/{}", captured.id))
        .json(&RetitlePieceRequest {
            title: "The Silent Loom".to_owned(),
        })
        .await
        .assert_status_ok();

    let listed = server
        .get("/pieces?project=project_1")
        .await
        .json::<Vec<PieceDTO>>();

    assert_eq!(listed[0].title, "The Silent Loom");
    assert_eq!(listed[0].version, 2, "the listing must not go stale");
}

#[tokio::test]
async fn an_attached_passage_shows_up_in_the_listing() {
    let server = a_server();
    let captured = a_piece(&server, "The Loom").await;

    server
        .put(&format!("/pieces/{}/passage", captured.id))
        .json(&AttachPassageRequest {
            passage: "passage_9".to_owned(),
        })
        .await
        .assert_status_ok();

    let listed = server
        .get("/pieces?project=project_1")
        .await
        .json::<Vec<PieceDTO>>();

    assert_eq!(listed[0].passage, Some("passage_9".to_owned()));
}

#[tokio::test]
async fn a_discarded_piece_leaves_the_listing() {
    let server = a_server();
    let kept = a_piece(&server, "The Loom").await;
    let discarded = a_piece(&server, "A false start").await;

    server
        .delete(&format!("/pieces/{}", discarded.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    let listed = server
        .get("/pieces?project=project_1")
        .await
        .json::<Vec<PieceDTO>>();

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, kept.id);
}

#[tokio::test]
async fn a_refused_change_leaves_the_listing_alone() {
    let server = a_server();
    let captured = a_piece(&server, "The Loom").await;

    server
        .patch(&format!("/pieces/{}", captured.id))
        .add_header("if-match", "\"99\"")
        .json(&RetitlePieceRequest {
            title: "Never happened".to_owned(),
        })
        .await
        .assert_status(StatusCode::PRECONDITION_FAILED);

    let listed = server
        .get("/pieces?project=project_1")
        .await
        .json::<Vec<PieceDTO>>();

    assert_eq!(
        listed[0].title, "The Loom",
        "a change that never landed must not reach the listing either"
    );
    assert_eq!(listed[0].version, 1);
}
