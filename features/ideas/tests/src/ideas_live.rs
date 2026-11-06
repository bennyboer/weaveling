use std::sync::Arc;

use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::Agent;
use ideas_contract::{CAPTURED, CaptureIdeaRequest, IdeaDTO, RetitleIdeaRequest};
use ideas_messaging::BroadcastIdeaChangesListener;
use live::Broadcaster;
use live::testing::Subscriber;
use messaging::Listener;
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

async fn a_follower_of(server: &TestServer, project: &str) -> Subscriber {
    let mut socket = Subscriber::on(server).await;
    socket
        .subscribe("ideas", "ideas", &[("project", project)])
        .await;

    socket
}

async fn told(socket: &mut Subscriber) -> Value {
    socket.frame().await
}

#[tokio::test]
async fn a_change_naming_no_idea_is_passed_over_rather_than_retried() {
    // given: a broadcast of changes to ideas
    let (wired, _server) = served();
    let broadcast =
        BroadcastIdeaChangesListener::new(wired.ideas.clone(), Broadcaster::new(), |project| {
            project
        });
    // and: a change naming no idea
    let naming_no_idea = eventpublishing::published(
        CAPTURED,
        "idea",
        "project_1",
        1,
        json!({ "version": 0, "name": "CAPTURED" }),
        &Agent::System,
        OffsetDateTime::UNIX_EPOCH,
    );

    // then: it is passed over without failing
    broadcast
        .handle(&naming_no_idea)
        .await
        .expect("nobody follows what is not an idea, so there is nothing to retry");
}

#[tokio::test]
async fn whoever_follows_a_project_is_told_each_change_to_its_ideas_by_the_idea_and_its_version() {
    // given: a follower of a project
    let (wired, server) = served();
    let mut socket = a_follower_of(&server, "project_1").await;

    // when: an idea of the project is captured, retitled and discarded
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

    // then: the follower is told the capture
    assert_eq!(
        told(&mut socket).await,
        json!({ "of": idea.id, "version": 1, "at": "1970-01-01T00:00:00Z", "event": { "name": "CAPTURED", "project": "project_1", "title": "The Loom" } })
    );
    // and: the retitling
    assert_eq!(
        told(&mut socket).await,
        json!({ "of": idea.id, "version": 2, "at": "1970-01-01T00:00:00Z", "event": { "name": "RETITLED", "title": "The Weft" } })
    );
    // and: the discarding
    assert_eq!(
        told(&mut socket).await,
        json!({ "of": idea.id, "version": 3, "at": "1970-01-01T00:00:00Z", "event": { "name": "DISCARDED" } }),
        "a discarded idea still belongs to its project, so its followers hear it go"
    );
}

#[tokio::test]
async fn another_project_s_ideas_are_not_told() {
    // given: a follower of a project
    let (wired, server) = served();
    let mut socket = a_follower_of(&server, "project_1").await;

    // when: an idea of another project is captured
    captured(&server, "project_2", "Theirs").await;
    // and: then one of the followed project
    let mine = captured(&server, "project_1", "Mine").await;
    wired.settle().await;

    // then: the follower is told only of its own project's idea
    assert_eq!(
        told(&mut socket).await["of"],
        mine.id,
        "the first thing told is about the project being followed"
    );
}
