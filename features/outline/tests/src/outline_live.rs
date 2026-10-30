use std::sync::Arc;

use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::Agent;
use live::testing::Subscriber;
use outline_contract::{AddSectionRequest, OpenOutlineRequest, OutlineDTO};
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

async fn an_open_outline(server: &TestServer, project: &str) -> String {
    let response = server
        .post("/outlines")
        .json(&OpenOutlineRequest {
            project: project.to_owned(),
        })
        .await;
    response.assert_status(StatusCode::OK);

    response.json::<OutlineDTO>().id
}

async fn a_section_added(server: &TestServer, outline: &str, title: &str) {
    server
        .post(&format!("/outlines/{outline}/sections"))
        .json(&AddSectionRequest {
            under: None,
            after: None,
            title: title.to_owned(),
        })
        .await
        .assert_status(StatusCode::CREATED);
}

async fn connected(server: &TestServer, outline: &str) -> Subscriber {
    let mut socket = Subscriber::on(server).await;
    socket
        .subscribe("outline", "outlines", &[("outline", outline)])
        .await;

    socket
}

async fn following(server: &TestServer, outline: &str) -> Subscriber {
    let mut socket = connected(server, outline).await;
    socket.frame().await;

    socket
}

async fn told(socket: &mut Subscriber) -> Value {
    socket.frame().await
}

#[tokio::test]
async fn whoever_follows_an_outline_is_told_each_change_by_its_version() {
    let (wired, server) = served();
    let outline = an_open_outline(&server, "project_1").await;
    wired.settle().await;
    let mut socket = following(&server, &outline).await;

    a_section_added(&server, &outline, "Part One").await;
    wired.settle().await;

    let frame = told(&mut socket).await;
    let now: OutlineDTO = server.get(&format!("/outlines/{outline}")).await.json();
    assert_eq!(frame["version"], now.version);
    assert_eq!(frame["event"]["name"], "SECTION_ADDED");
}

#[tokio::test]
async fn another_book_changing_is_not_told() {
    let (wired, server) = served();
    let mine = an_open_outline(&server, "project_1").await;
    let theirs = an_open_outline(&server, "project_2").await;
    wired.settle().await;
    let mut socket = following(&server, &mine).await;

    a_section_added(&server, &theirs, "Their part").await;
    a_section_added(&server, &mine, "My part").await;
    wired.settle().await;

    let frame = told(&mut socket).await;
    assert_eq!(
        frame["event"]["title"], "My part",
        "the first thing told is about the book being followed"
    );
}

#[tokio::test]
async fn whoever_follows_an_outline_is_first_told_the_version_it_stands_at() {
    let (wired, server) = served();
    let outline = an_open_outline(&server, "project_1").await;
    wired.settle().await;
    let now: OutlineDTO = server.get(&format!("/outlines/{outline}")).await.json();

    let mut socket = connected(&server, &outline).await;

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
async fn on_the_one_socket_an_outline_is_told_where_it_stands_then_each_change() {
    let (wired, server) = served();
    let outline = an_open_outline(&server, "project_1").await;
    wired.settle().await;
    let (_live, mut socket) = on_one_socket(&wired).await;

    socket
        .subscribe("outline", "outlines", &[("outline", &outline)])
        .await;
    let standing = told_on_one_socket(&mut socket).await;
    a_section_added(&server, &outline, "Part One").await;
    wired.settle().await;
    let added = told_on_one_socket(&mut socket).await;
    socket
        .subscribe("nowhere", "outlines", &[("outline", "not an outline")])
        .await;
    let nowhere = told_on_one_socket(&mut socket).await;

    assert_eq!(standing["id"], "outline");
    assert_eq!(
        standing["frame"],
        json!({ "version": 1 }),
        "told first where it stands"
    );
    assert_eq!(added["frame"]["of"], outline);
    assert_eq!(added["frame"]["version"], 2);
    assert_eq!(
        nowhere,
        json!({ "told": "ended", "id": "nowhere", "why": "not-there" })
    );
}
