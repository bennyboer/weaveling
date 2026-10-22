use std::sync::Arc;
use std::time::Duration;

use axum::Extension;
use axum::http::StatusCode;
use axum_test::{TestServer, TestWebSocket};
use clock::FixedClock;
use eventsourcing::Agent;
use projects_contract::{CreateProjectRequest, ProjectDTO, RenameProjectRequest};
use serde_json::{Value, json};
use time::OffsetDateTime;
use tokio::time::timeout;

use crate::wiring::{Wired, wired};

fn served() -> (Wired, TestServer) {
    let wired = wired(Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)));
    let server = TestServer::builder()
        .http_transport()
        .build(wired.routes.clone().layer(Extension(Agent::Local)));

    (wired, server)
}

async fn started(server: &TestServer, name: &str) -> ProjectDTO {
    let response = server
        .post("/projects")
        .json(&CreateProjectRequest {
            name: name.to_owned(),
            language: None,
            quotation: None,
            description: String::new(),
        })
        .await;
    response.assert_status(StatusCode::CREATED);

    response.json()
}

async fn following(server: &TestServer) -> TestWebSocket {
    server
        .get_websocket("/projects/live")
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
async fn whoever_follows_the_projects_is_told_each_change_to_any_of_them_by_the_project_and_its_version()
 {
    let (wired, server) = served();
    let mut socket = following(&server).await;

    let loom = started(&server, "The Loom").await;
    let weft = started(&server, "The Weft").await;
    server
        .patch(&format!("/projects/{}", loom.id))
        .json(&RenameProjectRequest {
            name: "The Heddle".to_owned(),
        })
        .await
        .assert_status_ok();
    server
        .delete(&format!("/projects/{}", weft.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    wired.settle().await;

    let mut frames = Vec::new();
    for _ in 0..4 {
        let frame = told(&mut socket).await;
        frames.push(json!([
            frame["of"],
            frame["version"],
            frame["event"]["name"]
        ]));
    }
    frames.sort_by_key(|frame| frame.to_string());
    let mut expected = vec![
        json!([loom.id, 1, "STARTED"]),
        json!([loom.id, 2, "RENAMED"]),
        json!([weft.id, 1, "STARTED"]),
        json!([weft.id, 2, "DELETED"]),
    ];
    expected.sort_by_key(|frame| frame.to_string());
    assert_eq!(frames, expected);
}

#[tokio::test]
async fn a_project_s_frame_says_when_it_changed() {
    let (wired, server) = served();
    let mut socket = following(&server).await;

    started(&server, "The Loom").await;
    wired.settle().await;

    let frame = told(&mut socket).await;
    assert_eq!(frame["at"], "1970-01-01T00:00:00Z");
    assert_eq!(frame["event"]["project_name"], "The Loom");
}

#[tokio::test]
async fn whoever_follows_one_project_is_told_its_changes_and_not_another_s() {
    let (wired, server) = served();
    let loom = started(&server, "The Loom").await;
    let weft = started(&server, "The Weft").await;
    wired.settle().await;
    let mut socket = server
        .get_websocket(&format!("/projects/live?project={}", loom.id))
        .await
        .into_websocket()
        .await;

    for (project, name) in [(&weft, "Theirs"), (&loom, "The Heddle")] {
        server
            .patch(&format!("/projects/{}", project.id))
            .json(&RenameProjectRequest {
                name: name.to_owned(),
            })
            .await
            .assert_status_ok();
    }
    wired.settle().await;

    let frame = told(&mut socket).await;
    assert_eq!(
        (frame["of"].clone(), frame["event"]["project_name"].clone()),
        (json!(loom.id), json!("The Heddle")),
        "the first thing told is about the project being followed"
    );
}
