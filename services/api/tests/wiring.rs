use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use axum_test::TestServer;
use clock::SystemClock;
use outbox::Cadence;
use serde_json::{Value, json};
use tower::ServiceExt;
use weaveling_service_api::{Adapters, Relays, app};

async fn new_app() -> Router {
    app(Adapters::in_memory(Arc::new(SystemClock)).await)
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
        .await
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
    let (status, body) = get("/api/service/health").await;

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
async fn the_scenes_feature_is_mounted_under_api() {
    let (status, body) = post_json("/api/scenes", r#"{"project":"project_1"}"#).await;

    assert_eq!(status, StatusCode::CREATED, "body was {body}");
    assert!(body.contains("scene_"), "body was {body}");
}

#[tokio::test]
async fn the_refusals_are_mounted_under_the_service() {
    let (status, body) = get("/api/service/refusals").await;

    assert_eq!(status, StatusCode::OK, "body was {body}");
    assert_eq!(body, "[]");
}

#[tokio::test]
async fn there_is_no_socket_for_one_scene_any_more() {
    let (status, _) = get("/api/scenes/sync/weaveling").await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "every scene travels on the one socket at /api/scenes/sync"
    );
}

#[tokio::test]
async fn the_outline_is_followed_live_under_api() {
    let (status, body) = get("/api/outlines/live?outline=outline_031VkO0hnpeQZUiAB7nDma").await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("upgrade"),
        "the live channel refuses a request that is not a socket: {body}"
    );
}

#[tokio::test]
async fn the_socket_is_found_under_the_feature_it_belongs_to() {
    let (status, _) = get("/api/sync").await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "every route starts with its feature, the scenes' socket too"
    );
}

#[tokio::test]
async fn the_two_features_do_not_shadow_each_other() {
    let (projects, _) = get("/api/projects").await;
    let (scenes, _) = post_json("/api/scenes", r#"{"project":"project_1"}"#).await;

    assert_eq!(projects, StatusCode::OK);
    assert_eq!(scenes, StatusCode::CREATED);
}

#[tokio::test]
async fn the_ideas_feature_is_mounted_under_api() {
    let (status, body) = post_json(
        "/api/ideas",
        r#"{"project":"project_1","title":"The Loom"}"#,
    )
    .await;

    assert_eq!(status, StatusCode::CREATED, "body was {body}");
    assert!(body.contains("idea_"), "body was {body}");
}

#[tokio::test]
async fn the_ideas_listing_is_mounted_under_api() {
    let (status, body) = get("/api/ideas?project=project_1").await;

    assert_eq!(status, StatusCode::OK, "body was {body}");
    assert_eq!(body, "[]");
}

#[tokio::test]
async fn every_feature_answers_without_shadowing_the_others() {
    let (projects, _) = get("/api/projects").await;
    let (scenes, _) = post_json("/api/scenes", r#"{"project":"project_1"}"#).await;
    let (ideas, _) = get("/api/ideas?project=project_1").await;

    assert_eq!(projects, StatusCode::OK);
    assert_eq!(scenes, StatusCode::CREATED);
    assert_eq!(ideas, StatusCode::OK);
}

#[tokio::test]
async fn the_live_scenes_are_settled_before_the_service_stops() {
    let adapters = Adapters::in_memory(Arc::new(SystemClock)).await;

    assert_eq!(
        adapters.settling().len(),
        1,
        "scenes holds edits and counts in memory, and stopping without settling them loses them"
    );
}

#[tokio::test]
async fn the_socket_for_many_scenes_is_mounted_under_api() {
    let (status, body) = get("/api/scenes/sync").await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("upgrade"),
        "the socket itself refuses a request that is not a socket, rather than a scene route          reading `sync` as a scene's id: {body}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn in_memory_a_captured_idea_travels_all_the_way_to_its_catalog() {
    let adapters = Adapters::in_memory(Arc::new(SystemClock)).await;
    let relays = Relays::started(
        adapters.outboxes(),
        adapters.consuming(),
        Cadence {
            deliver_every: Duration::from_millis(10),
            ..Cadence::default()
        },
    );
    let server = TestServer::new(app(adapters));
    let project = server
        .post("/api/projects")
        .json(&json!({ "name": "Kept in mind" }))
        .await
        .json::<Value>()["id"]
        .as_str()
        .expect("a project is made with an id")
        .to_owned();

    server
        .post("/api/ideas")
        .json(&json!({ "project": project, "title": "A girl in a wood" }))
        .await
        .assert_status(StatusCode::CREATED);

    let mut listed = Value::Null;
    for _ in 0..1_000 {
        listed = server
            .get(&format!("/api/ideas?project={project}"))
            .await
            .json();
        if listed.as_array().is_some_and(|ideas| !ideas.is_empty()) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        listed[0]["title"], "A girl in a wood",
        "in memory too, the catalog only hears of an idea through the outbox and its relay"
    );

    relays.stop().await;
}
