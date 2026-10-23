use std::sync::Arc;
use std::time::Duration;

use axum::Extension;
use axum::http::StatusCode;
use axum_test::{TestServer, TestWebSocket};
use clock::FixedClock;
use eventsourcing::Agent;
use ideas_contract::{CaptureIdeaRequest, IdeaDTO, RetitleIdeaRequest};
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

async fn captured(server: &TestServer, project: &str, title: &str) -> IdeaDTO {
    let response = server
        .post("/ideas")
        .json(&CaptureIdeaRequest {
            project: project.to_owned(),
            title: title.to_owned(),
        })
        .await;
    response.assert_status(StatusCode::CREATED);

    response.json()
}

async fn following(server: &TestServer, project: &str) -> TestWebSocket {
    server
        .get_websocket(&format!("/ideas/live?project={project}"))
        .await
        .into_websocket()
        .await
}

async fn told(socket: &mut TestWebSocket) -> Value {
    let frame = tokio::time::timeout(Duration::from_secs(1), socket.receive_text())
        .await
        .expect("a change is told while it is fresh");

    serde_json::from_str(&frame).expect("a frame is JSON")
}

#[tokio::test]
async fn whoever_follows_a_project_is_told_each_change_to_its_ideas_by_the_idea_and_its_version() {
    let (wired, server) = served();
    let mut socket = following(&server, "project_1").await;

    let idea = captured(&server, "project_1", "The Loom").await;
    server
        .patch(&format!("/ideas/{}", idea.id))
        .json(&RetitleIdeaRequest {
            title: "The Weft".to_owned(),
        })
        .await
        .assert_status_ok();
    server
        .delete(&format!("/ideas/{}", idea.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    wired.settle().await;

    assert_eq!(
        told(&mut socket).await,
        json!({ "of": idea.id, "version": 1, "at": "1970-01-01T00:00:00Z", "event": { "name": "CAPTURED", "project": "project_1", "title": "The Loom" } })
    );
    assert_eq!(
        told(&mut socket).await,
        json!({ "of": idea.id, "version": 2, "at": "1970-01-01T00:00:00Z", "event": { "name": "RETITLED", "title": "The Weft" } })
    );
    assert_eq!(
        told(&mut socket).await,
        json!({ "of": idea.id, "version": 3, "at": "1970-01-01T00:00:00Z", "event": { "name": "DISCARDED" } }),
        "a discarded idea still belongs to its project, so its followers hear it go"
    );
}

#[tokio::test]
async fn another_project_s_ideas_are_not_told() {
    let (wired, server) = served();
    let mut socket = following(&server, "project_1").await;

    captured(&server, "project_2", "Theirs").await;
    let mine = captured(&server, "project_1", "Mine").await;
    wired.settle().await;

    assert_eq!(
        told(&mut socket).await["of"],
        mine.id,
        "the first thing told is about the project being followed"
    );
}
