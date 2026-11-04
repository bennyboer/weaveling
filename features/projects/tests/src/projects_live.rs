use std::sync::Arc;

use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::Agent;
use live::testing::Subscriber;
use projects_contract::{CreateProjectRequest, ProjectDTO, RenameProjectRequest};
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

async fn following(server: &TestServer) -> Subscriber {
    let mut socket = Subscriber::on(server).await;
    socket
        .subscribe("list", "projects", &[("mine", "true")])
        .await;

    socket
}

async fn told(socket: &mut Subscriber) -> Value {
    socket.frame().await
}

#[tokio::test]
async fn whoever_follows_the_projects_is_told_each_change_to_any_of_them_by_the_project_and_its_version()
 {
    // given: someone following the projects
    let (wired, server) = served();
    let mut socket = following(&server).await;

    // when: two projects are started
    let loom = started(&server, "The Loom").await;
    let weft = started(&server, "The Weft").await;
    // and: one is renamed and the other deleted
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

    // then: they are told each change by the project and its version
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
        json!([weft.id, 2, "TRASHED"]),
    ];
    expected.sort_by_key(|frame| frame.to_string());
    assert_eq!(frames, expected);
}

#[tokio::test]
async fn a_project_s_frame_says_when_it_changed() {
    // given: someone following the projects
    let (wired, server) = served();
    let mut socket = following(&server).await;

    // when: a project is started
    started(&server, "The Loom").await;
    wired.settle().await;

    // then: they are told when it was started and what it is called
    let frame = told(&mut socket).await;
    assert_eq!(frame["at"], "1970-01-01T00:00:00Z");
    assert_eq!(frame["event"]["project_name"], "The Loom");
}

#[tokio::test]
async fn whoever_follows_one_project_is_told_its_changes_and_not_another_s() {
    // given: two projects
    let (wired, server) = served();
    let loom = started(&server, "The Loom").await;
    let weft = started(&server, "The Weft").await;
    wired.settle().await;
    // and: someone following one of them
    let mut socket = Subscriber::on(&server).await;
    socket
        .subscribe("loom", "projects", &[("project", &loom.id)])
        .await;

    // when: the other is renamed, and then the followed one
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

    // then: the first they are told of is the followed one's renaming
    let frame = told(&mut socket).await;
    assert_eq!(
        (frame["of"].clone(), frame["event"]["project_name"].clone()),
        (json!(loom.id), json!("The Heddle")),
        "the first thing told is about the project being followed"
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
async fn on_the_one_socket_the_list_hears_every_project_and_one_project_its_own() {
    // given: a project
    let (wired, server) = served();
    let loom = started(&server, "The Loom").await;
    wired.settle().await;
    // and: one socket following the list and that project
    let (_live, mut socket) = on_one_socket(&wired).await;
    socket
        .subscribe("list", "projects", &[("mine", "true")])
        .await;
    socket
        .subscribe("loom", "projects", &[("project", &loom.id)])
        .await;

    // when: another project is started
    let weft = started(&server, "The Weft").await;
    wired.settle().await;
    let listed = told_on_one_socket(&mut socket).await;
    // and: the followed project is renamed
    server
        .patch(&format!("/projects/{}", loom.id))
        .json(&json!({ "name": "The Loom Again" }))
        .await
        .assert_status_ok();
    wired.settle().await;
    let mut renamed = [
        told_on_one_socket(&mut socket).await,
        told_on_one_socket(&mut socket).await,
    ];
    renamed.sort_by_key(|told| told["id"].as_str().unwrap_or_default().to_owned());
    // and: something that is no project is followed
    socket.subscribe("nothing", "projects", &[]).await;

    // then: only the list hears of the other project
    assert_eq!(
        listed["id"], "list",
        "the list hears another project's start, the one project does not"
    );
    assert_eq!(listed["frame"]["of"], weft.id);
    // and: both hear of the renaming
    assert_eq!(
        renamed
            .iter()
            .map(|told| (told["id"].clone(), told["frame"]["of"].clone()))
            .collect::<Vec<_>>(),
        vec![
            (json!("list"), json!(loom.id)),
            (json!("loom"), json!(loom.id))
        ]
    );
    // and: following nothing there is ended
    assert_eq!(
        told_on_one_socket(&mut socket).await,
        json!({ "told": "ended", "id": "nothing", "why": "not-there" })
    );
}
