use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use authorizing::{AccessChanges, AccessWatch, Act, Authorizer, AuthorizerError, Readable};
use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use live::testing::Subscriber;
use live_contract::{ACCESS_CHANGED, ToldDTO};
use projects_contract::{CreateProjectRequest, ProjectDTO, RenameProjectRequest};
use serde_json::{Value, json};
use time::OffsetDateTime;

use crate::wiring;
use crate::wiring::Wired;

#[derive(Default)]
struct Readers {
    readable: Mutex<BTreeSet<String>>,
    restorable: Mutex<BTreeSet<String>>,
    down: Mutex<bool>,
    changes: AccessChanges,
}

impl Readers {
    fn let_restore(&self, project: &str) {
        self.restorable
            .lock()
            .expect("never poisoned")
            .insert(project.to_owned());
    }

    fn let_read(&self, project: &str) {
        self.readable
            .lock()
            .expect("never poisoned")
            .insert(project.to_owned());
    }

    fn stop_reading(&self, project: &str) {
        self.readable
            .lock()
            .expect("never poisoned")
            .remove(project);
        self.access_changed(project);
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

        Ok(match act {
            Act::Read => self
                .readable
                .lock()
                .expect("never poisoned")
                .contains(project),
            Act::Restore => self
                .restorable
                .lock()
                .expect("never poisoned")
                .contains(project),
            Act::Write | Act::Manage => true,
        })
    }

    async fn readable_by(&self, _agent: &Agent) -> Result<Readable, AuthorizerError> {
        Ok(Readable::Only(
            self.readable.lock().expect("never poisoned").clone(),
        ))
    }

    fn access_changed(&self, project: &str) {
        self.changes.tell(project);
    }

    fn watch_access(&self, project: &str) -> AccessWatch {
        self.changes.watch(project)
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

async fn renaming(server: &TestServer, project: &ProjectDTO, name: &str) {
    server
        .patch(&format!("/projects/{}", project.id))
        .json(&RenameProjectRequest {
            name: name.to_owned(),
        })
        .await
        .assert_status_ok();
}

async fn following(server: &TestServer, about: &[(&str, &str)]) -> Subscriber {
    let mut socket = Subscriber::on(server).await;
    socket.subscribe("projects", "projects", about).await;

    socket
}

async fn told(socket: &mut Subscriber) -> Value {
    socket.frame().await
}

#[tokio::test]
async fn whoever_follows_the_projects_is_told_only_of_those_they_may_read() {
    // given: someone following the projects
    let guarded = guarded();
    let mut socket = following(&guarded.server, &[("mine", "true")]).await;
    // and: two projects, of which they may read one
    let loom = started(&guarded.server, "The Loom").await;
    let weft = started(&guarded.server, "The Weft").await;
    guarded.readers.let_read(&loom.id);

    // when: both are renamed
    renaming(&guarded.server, &weft, "The Warp").await;
    renaming(&guarded.server, &loom, "The Heddle").await;
    guarded.wired.settle().await;

    // then: they are told only of the one they may read
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
async fn whoever_may_restore_a_project_from_the_trash_is_told_of_it_though_they_may_not_read_it() {
    // given: someone following the projects
    let guarded = guarded();
    let mut socket = following(&guarded.server, &[("mine", "true")]).await;
    // and: two projects, of which they may restore one but read neither
    let loom = started(&guarded.server, "The Loom").await;
    let weft = started(&guarded.server, "The Weft").await;
    guarded.readers.let_restore(&loom.id);

    // when: both are trashed
    for project in [&weft, &loom] {
        guarded
            .server
            .delete(&format!("/projects/{}", project.id))
            .await
            .assert_status(StatusCode::NO_CONTENT);
    }
    guarded.wired.settle().await;

    // then: they are told only of the one they may restore
    let mut frames = Vec::new();
    for _ in 0..2 {
        let frame = told(&mut socket).await;
        frames.push(json!([frame["of"], frame["event"]["name"]]));
    }
    assert_eq!(
        frames,
        vec![json!([loom.id, "STARTED"]), json!([loom.id, "TRASHED"])],
        "a trashed project is read by nobody, yet its owners see their trash change, and nothing of the Weft"
    );
}

#[tokio::test]
async fn the_list_followed_ends_once_whom_to_tell_cannot_be_known() {
    // given: someone following the projects
    let guarded = guarded();
    let mut socket = following(&guarded.server, &[("mine", "true")]).await;
    // and: a project started
    started(&guarded.server, "The Loom").await;

    // when: whom to tell can no longer be learned
    guarded.readers.go_down();
    guarded.wired.settle().await;

    // then: their following ends as failed
    assert_eq!(
        socket.ended().await,
        "failed",
        "a project it cannot tell about may be one the follower needed, so the subscription ends and its client asks again"
    );
}

#[tokio::test]
async fn whoever_follows_one_project_hears_it_but_only_if_they_may_read_it() {
    // given: two projects, of which one may be read
    let guarded = guarded();
    let loom = started(&guarded.server, "The Loom").await;
    let weft = started(&guarded.server, "The Weft").await;
    guarded.readers.let_read(&loom.id);
    guarded.wired.settle().await;

    // expect: following the other ends as not there
    assert_eq!(
        following(&guarded.server, &[("project", &weft.id)])
            .await
            .ended()
            .await,
        "not-there"
    );

    // when: the readable one is followed and renamed
    let mut socket = following(&guarded.server, &[("project", &loom.id)]).await;
    renaming(&guarded.server, &loom, "The Heddle").await;
    guarded.wired.settle().await;

    // then: its renaming is told
    let frame = told(&mut socket).await;
    assert_eq!(json!([frame["of"], frame["version"]]), json!([loom.id, 2]));
}

#[tokio::test]
async fn whether_one_may_follow_a_project_not_being_known_is_a_failure() {
    // given: a project
    let guarded = guarded();
    let loom = started(&guarded.server, "The Loom").await;
    // and: who may read it cannot be learned
    guarded.readers.go_down();

    // expect: following it ends as failed
    assert_eq!(
        following(&guarded.server, &[("project", &loom.id)])
            .await
            .ended()
            .await,
        "failed"
    );
}

#[tokio::test]
async fn whoever_may_no_longer_read_the_project_they_follow_is_closed_out() {
    // given: someone following a project they may read
    let guarded = guarded();
    let loom = started(&guarded.server, "The Loom").await;
    guarded.readers.let_read(&loom.id);
    let mut socket = following(&guarded.server, &[("project", &loom.id)]).await;

    // when: they may no longer read it
    guarded.readers.stop_reading(&loom.id);

    // then: their following ends as access lost
    assert_eq!(socket.ended().await, "access-lost");
}

#[tokio::test]
async fn whoever_may_restore_the_project_they_follow_hears_it_trashed_though_reading_ended_first() {
    // given: someone following a project they may read and restore
    let guarded = guarded();
    let loom = started(&guarded.server, "The Loom").await;
    guarded.readers.let_read(&loom.id);
    guarded.readers.let_restore(&loom.id);
    guarded.wired.settle().await;
    let mut socket = following(&guarded.server, &[("project", &loom.id)]).await;

    // when: they may no longer read it
    guarded.readers.stop_reading(&loom.id);
    // then: they are told their access changed
    assert_eq!(
        socket.told().await,
        ToldDTO::Notice {
            id: "projects".to_owned(),
            notice: ACCESS_CHANGED.to_owned(),
        }
    );

    // when: it is trashed
    guarded
        .server
        .delete(&format!("/projects/{}", loom.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    guarded.wired.settle().await;

    // then: they are told of it
    let frame = told(&mut socket).await;
    assert_eq!(
        json!([frame["of"], frame["event"]["name"]]),
        json!([loom.id, "TRASHED"]),
        "its members hear of the trash apart from the project, so an owner's reading may end before the trash is told"
    );
}
