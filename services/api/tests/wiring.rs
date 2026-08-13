use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use clock::SystemClock;
use projects_store::InMemoryProjectStore;
use tower::ServiceExt;
use weaveling_service_api::app;

fn new_app() -> Router {
    app(Arc::new(InMemoryProjectStore::new()), Arc::new(SystemClock))
}

async fn get(path: &str) -> (StatusCode, String) {
    let request = Request::builder()
        .uri(path)
        .body(Body::empty())
        .expect("should build the request");
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
