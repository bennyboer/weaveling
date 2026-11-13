use std::sync::Arc;

use access::{HearsAccess, Role};
use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, Version};
use live::testing::Subscriber;
use live_contract::{ACCESS_CHANGED, ToldDTO};
use projects_contract::{CreateProjectRequest, ProjectDTO};
use serde_json::{Value, json};
use time::OffsetDateTime;

use crate::wiring;
use crate::wiring::Wired;

const ADA: &str = "account_ada";

struct Guarded {
    wired: Wired,
    server: TestServer,
}

fn ada() -> Agent {
    Agent::Account(AccountId::from(ADA))
}

fn guarded() -> Guarded {
    let wired = wiring::wired_with_accounts(Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)));
    let server = TestServer::builder()
        .http_transport()
        .build(wired.routes.clone().layer(Extension(ada())));

    Guarded { wired, server }
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

async fn renaming(wired: &Wired, project: &ProjectDTO, name: &str) {
    wired
        .projects
        .rename(&project.id, name, None, &Agent::System)
        .await
        .expect("the system renames any project");
}

async fn trashing(wired: &Wired, project: &ProjectDTO) {
    wired
        .projects
        .trash(&project.id, None, &Agent::System)
        .await
        .expect("the system trashes any project");
}

async fn trash_heard(wired: &Wired, project: &ProjectDTO) {
    wired
        .projects
        .remember_trash(None, &project.id, Version::of(2), true)
        .await
        .expect("remembering the trash should succeed");
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
    guarded.wired.give(&loom.id, ADA, Some(Role::Reader)).await;

    // when: both are renamed
    renaming(&guarded.wired, &weft, "The Warp").await;
    renaming(&guarded.wired, &loom, "The Heddle").await;
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
    guarded.wired.give(&loom.id, ADA, Some(Role::Owner)).await;

    // when: both are trashed
    for project in [&weft, &loom] {
        trashing(&guarded.wired, project).await;
    }
    // and: the loom's trash is heard before its trashing is told, so it may only be restored
    trash_heard(&guarded.wired, &loom).await;
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
    guarded.wired.access.go_down();
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
    guarded.wired.give(&loom.id, ADA, Some(Role::Reader)).await;
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
    renaming(&guarded.wired, &loom, "The Heddle").await;
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
    guarded.wired.access.go_down();

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
    guarded.wired.give(&loom.id, ADA, Some(Role::Reader)).await;
    let mut socket = following(&guarded.server, &[("project", &loom.id)]).await;

    // when: they may no longer read it, having left it
    guarded.wired.give(&loom.id, ADA, None).await;

    // then: their following ends as access lost
    assert_eq!(socket.ended().await, "access-lost");
}

#[tokio::test]
async fn whoever_may_restore_the_project_they_follow_hears_it_trashed_though_reading_ended_first() {
    // given: someone following a project they may read and restore
    let guarded = guarded();
    let loom = started(&guarded.server, "The Loom").await;
    guarded.wired.give(&loom.id, ADA, Some(Role::Owner)).await;
    guarded.wired.settle().await;
    let mut socket = following(&guarded.server, &[("project", &loom.id)]).await;

    // when: it is trashed
    trashing(&guarded.wired, &loom).await;
    // and: its trash is heard before its trashing is told, so they may no longer read it
    trash_heard(&guarded.wired, &loom).await;
    // then: they are told their access changed
    assert_eq!(
        socket.told().await,
        ToldDTO::Notice {
            id: "projects".to_owned(),
            notice: ACCESS_CHANGED.to_owned(),
        }
    );

    // when: its trashing is told
    guarded.wired.settle().await;

    // then: they are told of it, its trash heard once more on the way
    let frame = loop {
        match socket.told().await {
            ToldDTO::Frame { frame, .. } => {
                break serde_json::to_value(frame).expect("a frame is plain data");
            }
            ToldDTO::Notice { notice, .. } if notice == ACCESS_CHANGED => {}
            other => panic!("a frame, not {other:?}"),
        }
    };
    assert_eq!(
        json!([frame["of"], frame["event"]["name"]]),
        json!([loom.id, "TRASHED"]),
        "who may do what hears of the trash apart from the project's followers, so an owner's reading may end before the trash is told"
    );
}
