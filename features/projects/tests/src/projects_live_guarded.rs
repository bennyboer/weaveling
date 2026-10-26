use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use authorizing::{Act, Authorizer, AuthorizerError, Readable};
use axum::Extension;
use axum::http::StatusCode;
use axum_test::{TestServer, TestWebSocket};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use projects_contract::{CreateProjectRequest, ProjectDTO, RenameProjectRequest};
use serde_json::{Value, json};
use time::OffsetDateTime;

use crate::wiring;
use crate::wiring::Wired;

#[derive(Default)]
struct Readers {
    readable: Mutex<BTreeSet<String>>,
    down: Mutex<bool>,
}

impl Readers {
    fn let_read(&self, project: &str) {
        self.readable
            .lock()
            .expect("never poisoned")
            .insert(project.to_owned());
    }

    fn go_down(&self) {
        *self.down.lock().expect("never poisoned") = true;
    }
}

#[async_trait]
impl Authorizer for Readers {
    async fn may(&self, _agent: &Agent, act: Act, project: &str) -> Result<bool, AuthorizerError> {
        if *self.down.lock().expect("never poisoned") {
            return Err(AuthorizerError::Unreachable("the list is down".into()));
        }

        Ok(act != Act::Read
            || self
                .readable
                .lock()
                .expect("never poisoned")
                .contains(project))
    }

    async fn readable_by(&self, _agent: &Agent) -> Result<Readable, AuthorizerError> {
        Ok(Readable::Only(
            self.readable.lock().expect("never poisoned").clone(),
        ))
    }
}

struct Guarded {
    wired: Wired,
    server: TestServer,
    readers: Arc<Readers>,
}

fn ada() -> Agent {
    Agent::Account(AccountId::from("account_ada"))
}

fn guarded() -> Guarded {
    let readers = Arc::new(Readers::default());
    let wired = wiring::wired_guarded_by(
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        readers.clone(),
    );
    let server = TestServer::builder()
        .http_transport()
        .build(wired.routes.clone().layer(Extension(ada())));

    Guarded {
        wired,
        server,
        readers,
    }
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

async fn renamed(server: &TestServer, project: &ProjectDTO, name: &str) {
    server
        .patch(&format!("/projects/{}", project.id))
        .json(&RenameProjectRequest {
            name: name.to_owned(),
        })
        .await
        .assert_status_ok();
}

async fn following(server: &TestServer, path: &str) -> TestWebSocket {
    server.get_websocket(path).await.into_websocket().await
}

async fn told(socket: &mut TestWebSocket) -> Value {
    let frame = tokio::time::timeout(Duration::from_secs(1), socket.receive_text())
        .await
        .expect("a change is told while it is fresh");

    serde_json::from_str(&frame).expect("a frame is JSON")
}

#[tokio::test]
async fn whoever_follows_the_projects_is_told_only_of_those_they_may_read() {
    let guarded = guarded();
    let mut socket = following(&guarded.server, "/projects/live").await;
    let loom = started(&guarded.server, "The Loom").await;
    let weft = started(&guarded.server, "The Weft").await;
    guarded.readers.let_read(&loom.id);

    renamed(&guarded.server, &weft, "The Warp").await;
    renamed(&guarded.server, &loom, "The Heddle").await;
    guarded.wired.settle().await;

    let mut frames = Vec::new();
    for _ in 0..2 {
        let frame = told(&mut socket).await;
        frames.push(json!([frame["of"], frame["version"]]));
    }
    assert_eq!(
        frames,
        vec![json!([loom.id, 1]), json!([loom.id, 2])],
        "nothing of the Weft is told, which this follower may not read"
    );
}

#[tokio::test]
async fn the_list_followed_ends_once_whom_to_tell_cannot_be_known() {
    let guarded = guarded();
    let mut socket = following(&guarded.server, "/projects/live").await;
    started(&guarded.server, "The Loom").await;

    guarded.readers.go_down();
    guarded.wired.settle().await;

    assert!(
        tokio::spawn(async move { socket.receive_text().await })
            .await
            .is_err(),
        "a project it cannot tell about may be one the follower needed, so the socket ends and its client reloads"
    );
}

#[tokio::test]
async fn whoever_follows_one_project_hears_it_but_only_if_they_may_read_it() {
    let guarded = guarded();
    let loom = started(&guarded.server, "The Loom").await;
    let weft = started(&guarded.server, "The Weft").await;
    guarded.readers.let_read(&loom.id);
    guarded.wired.settle().await;

    guarded
        .server
        .get_websocket(&format!("/projects/live?project={}", weft.id))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    let mut socket = following(
        &guarded.server,
        &format!("/projects/live?project={}", loom.id),
    )
    .await;
    renamed(&guarded.server, &loom, "The Heddle").await;
    guarded.wired.settle().await;

    let frame = told(&mut socket).await;
    assert_eq!(json!([frame["of"], frame["version"]]), json!([loom.id, 2]));
}

#[tokio::test]
async fn whether_one_may_follow_a_project_not_being_known_is_a_failure() {
    let guarded = guarded();
    let loom = started(&guarded.server, "The Loom").await;
    guarded.readers.go_down();

    guarded
        .server
        .get_websocket(&format!("/projects/live?project={}", loom.id))
        .await
        .assert_status(StatusCode::INTERNAL_SERVER_ERROR);
}
