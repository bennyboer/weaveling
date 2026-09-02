use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use boards_catalog::InMemoryBoardCatalog;
use clock::SystemClock;
use eventsourcing::InMemoryEventStore;
use passages_store::InMemoryPassageStore;
use pieces_catalog::InMemoryPieceCatalog;
use projects_store::InMemoryProjectStore;
use tower::ServiceExt;
use weaveling_service_api::app;

fn new_app() -> Router {
    app(
        Arc::new(InMemoryProjectStore::new()),
        Arc::new(InMemoryPassageStore::new()),
        Arc::new(InMemoryEventStore::new()),
        Arc::new(InMemoryPieceCatalog::new()),
        Arc::new(InMemoryEventStore::new()),
        Arc::new(InMemoryBoardCatalog::new()),
        Arc::new(SystemClock),
    )
}

async fn get(path: &str) -> (StatusCode, String) {
    reply(
        Request::builder()
            .uri(path)
            .body(Body::empty())
            .expect("should build the request"),
    )
    .await
}

async fn post(path: &str) -> (StatusCode, String) {
    reply(
        Request::builder()
            .method("POST")
            .uri(path)
            .body(Body::empty())
            .expect("should build the request"),
    )
    .await
}

async fn post_json(path: &str, body: &str) -> (StatusCode, String) {
    reply(
        Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from(body.to_owned()))
            .expect("should build the request"),
    )
    .await
}

async fn reply(request: Request<Body>) -> (StatusCode, String) {
    let response = new_app()
        .oneshot(request)
        .await
        .expect("the app should respond");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("the body should be readable");

    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn the_health_check_answers_ok() {
    let (status, body) = get("/api/health").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn the_projects_feature_is_mounted_under_api() {
    let (status, body) = get("/api/projects").await;

    assert_eq!(status, StatusCode::OK, "body was {body}");
    assert_eq!(body, "[]");
}

#[tokio::test]
async fn feature_routes_are_not_reachable_outside_the_api_prefix() {
    let (status, _) = get("/projects").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_unknown_path_answers_404() {
    let (status, _) = get("/api/nope").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_passages_feature_is_mounted_under_api() {
    let (status, body) = post("/api/passages").await;

    assert_eq!(status, StatusCode::CREATED, "body was {body}");
    assert!(body.contains("passage_"), "body was {body}");
}

#[tokio::test]
async fn the_sync_socket_is_mounted_under_api() {
    let (status, _) = get("/api/sync/weaveling").await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a malformed id must be refused by the route, not answered with 404 by the router"
    );
}

#[tokio::test]
async fn the_two_features_do_not_shadow_each_other() {
    let (projects, _) = get("/api/projects").await;
    let (passages, _) = post("/api/passages").await;

    assert_eq!(projects, StatusCode::OK);
    assert_eq!(passages, StatusCode::CREATED);
}

#[tokio::test]
async fn the_pieces_feature_is_mounted_under_api() {
    let (status, body) = post_json(
        "/api/pieces",
        r#"{"project":"project_1","title":"The Loom"}"#,
    )
    .await;

    assert_eq!(status, StatusCode::CREATED, "body was {body}");
    assert!(body.contains("piece_"), "body was {body}");
}

#[tokio::test]
async fn the_pieces_listing_is_mounted_under_api() {
    let (status, body) = get("/api/pieces?project=project_1").await;

    assert_eq!(status, StatusCode::OK, "body was {body}");
    assert_eq!(body, "[]");
}

#[tokio::test]
async fn every_feature_answers_without_shadowing_the_others() {
    let (projects, _) = get("/api/projects").await;
    let (passages, _) = post("/api/passages").await;
    let (pieces, _) = get("/api/pieces?project=project_1").await;

    assert_eq!(projects, StatusCode::OK);
    assert_eq!(passages, StatusCode::CREATED);
    assert_eq!(pieces, StatusCode::OK);
}
