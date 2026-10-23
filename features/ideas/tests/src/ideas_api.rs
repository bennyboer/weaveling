use std::sync::Arc;

use axum::Extension;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum_test::{TestResponse, TestServer};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, AggregateId, EventStore, Version};
use ideas_contract::{CaptureIdeaRequest, IdeaDTO, RetitleIdeaRequest, refused};
use ideas_core::{IdeaTitle, KIND};

use crate::wiring;
use crate::wiring::Wired;
use serde_json::Value;
use time::{Duration, OffsetDateTime};

const UNKNOWN_ID: &str = "idea_031VkO0hnpeQZUiAB7nDma";

fn refused_as(response: TestResponse, status: StatusCode, kind: &str) -> Value {
    response.assert_status(status);
    assert_eq!(response.header(CONTENT_TYPE), "application/problem+json");
    let told: Value = response.json();
    assert_eq!(told["type"], kind, "{told}");
    assert_eq!(told["status"], status.as_u16(), "{told}");

    told
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

struct Serving {
    server: TestServer,
    wired: Wired,
}

impl std::ops::Deref for Serving {
    type Target = TestServer;

    fn deref(&self) -> &TestServer {
        &self.server
    }
}

impl Serving {
    async fn listed(&self, project: &str) -> Vec<IdeaDTO> {
        self.wired.settle().await;

        self.server
            .get(&format!("/ideas?project={project}"))
            .await
            .json()
    }
}

fn a_server() -> Serving {
    a_server_acting_as(Agent::Local)
}

fn a_server_acting_as(acting: Agent) -> Serving {
    let wired = wiring::wired(Arc::new(FixedClock::new(at(1_700_000_000))));
    let server = TestServer::new(wired.routes.clone().layer(Extension(acting)));

    Serving { server, wired }
}

fn a_capture(title: &str) -> CaptureIdeaRequest {
    CaptureIdeaRequest {
        project: "project_1".to_owned(),
        title: title.to_owned(),
    }
}

async fn an_idea(server: &Serving, title: &str) -> IdeaDTO {
    let response = server.post("/ideas").json(&a_capture(title)).await;
    response.assert_status(StatusCode::CREATED);

    response.json()
}

#[tokio::test]
async fn a_captured_idea_comes_back_with_a_prefixed_id() {
    let server = a_server();

    let captured = an_idea(&server, "The Loom").await;

    assert!(
        captured.id.starts_with("idea_"),
        "an id should say what it is: {}",
        captured.id
    );
    assert_eq!(captured.title, "The Loom");
    assert_eq!(captured.project, "project_1");
}

#[tokio::test]
async fn an_idea_is_captured_by_whoever_the_request_says_acts() {
    let acting = Agent::Account(AccountId::from("account_7"));
    let server = a_server_acting_as(acting.clone());

    let captured = an_idea(&server, "The Loom").await;

    let recorded = server
        .wired
        .store
        .read_from(
            &AggregateId::from(captured.id.as_str()),
            KIND,
            Version::ZERO,
        )
        .await
        .expect("reading should succeed");
    assert_eq!(
        recorded[0].metadata.agent, acting,
        "a change has to say who made it, or a shared book cannot tell its authors apart"
    );
}

#[tokio::test]
async fn a_request_nobody_is_said_to_act_in_is_not_served() {
    let wired = wiring::wired(Arc::new(FixedClock::new(at(1_700_000_000))));
    let server = TestServer::new(wired.routes.clone());

    server
        .post("/ideas")
        .json(&a_capture("The Loom"))
        .await
        .assert_status(StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn an_idea_may_be_captured_with_no_title() {
    let server = a_server();

    let captured = an_idea(&server, "").await;

    assert_eq!(captured.title, "");
}

#[tokio::test]
async fn two_captured_ideas_are_distinct() {
    let server = a_server();

    let one = an_idea(&server, "The Loom").await;
    let other = an_idea(&server, "The Loom").await;

    assert_ne!(one.id, other.id);
}

#[tokio::test]
async fn a_captured_idea_can_be_fetched_again() {
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    let response = server.get(&format!("/ideas/{}", captured.id)).await;

    response.assert_status_ok();
    assert_eq!(response.json::<IdeaDTO>(), captured);
}

#[tokio::test]
async fn an_idea_nobody_captured_is_not_found() {
    let server = a_server();

    refused_as(
        server.get(&format!("/ideas/{UNKNOWN_ID}")).await,
        StatusCode::NOT_FOUND,
        "not-found",
    );
}

#[tokio::test]
async fn an_id_of_another_kind_is_a_bad_request() {
    let server = a_server();

    refused_as(
        server.get("/ideas/scene_031VkO0hnpeQZUiAB7nDma").await,
        StatusCode::BAD_REQUEST,
        refused::ID_INVALID,
    );
}

#[tokio::test]
async fn an_id_without_its_prefix_is_a_bad_request() {
    let server = a_server();

    refused_as(
        server.get("/ideas/031VkO0hnpeQZUiAB7nDma").await,
        StatusCode::BAD_REQUEST,
        refused::ID_INVALID,
    );
}

#[tokio::test]
async fn an_idea_can_be_retitled() {
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    let response = server
        .patch(&format!("/ideas/{}", captured.id))
        .json(&RetitleIdeaRequest {
            title: "The Silent Loom".to_owned(),
        })
        .await;

    response.assert_status_ok();
    assert_eq!(response.json::<IdeaDTO>().title, "The Silent Loom");
}

#[tokio::test]
async fn a_title_the_domain_refuses_is_a_bad_request() {
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    let told = refused_as(
        server
            .patch(&format!("/ideas/{}", captured.id))
            .json(&RetitleIdeaRequest {
                title: "a".repeat(IdeaTitle::MAX_CHARS + 1),
            })
            .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::TITLE_TOO_LONG,
    );
    assert_eq!(told["max"], IdeaTitle::MAX_CHARS);
    assert_eq!(told["actual"], IdeaTitle::MAX_CHARS + 1);
}

#[tokio::test]
async fn a_discarded_idea_is_gone_from_the_authors_point_of_view() {
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    server
        .delete(&format!("/ideas/{}", captured.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    refused_as(
        server
            .patch(&format!("/ideas/{}", captured.id))
            .json(&RetitleIdeaRequest {
                title: "Too late".to_owned(),
            })
            .await,
        StatusCode::CONFLICT,
        refused::DISCARDED,
    );
}

#[tokio::test]
async fn discarding_something_that_was_never_captured_is_not_found() {
    let server = a_server();

    refused_as(
        server.delete(&format!("/ideas/{UNKNOWN_ID}")).await,
        StatusCode::NOT_FOUND,
        "not-found",
    );
}

#[tokio::test]
async fn an_idea_reports_the_version_it_stands_at() {
    let server = a_server();

    let captured = an_idea(&server, "The Loom").await;

    assert_eq!(
        captured.version, 1,
        "a captured idea stands at its first version"
    );
}

#[tokio::test]
async fn every_response_carries_the_version_as_an_etag() {
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    let response = server.get(&format!("/ideas/{}", captured.id)).await;

    assert_eq!(
        response.header("etag").to_str().expect("an ascii etag"),
        "\"1\"",
        "a caller needs somewhere to read the version it is looking at"
    );
}

#[tokio::test]
async fn a_change_moves_the_version_on() {
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    let response = server
        .patch(&format!("/ideas/{}", captured.id))
        .json(&RetitleIdeaRequest {
            title: "The Silent Loom".to_owned(),
        })
        .await;

    assert_eq!(response.json::<IdeaDTO>().version, 2);
}

#[tokio::test]
async fn a_change_from_the_version_the_caller_saw_is_accepted() {
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    server
        .patch(&format!("/ideas/{}", captured.id))
        .add_header("if-match", "\"1\"")
        .json(&RetitleIdeaRequest {
            title: "The Silent Loom".to_owned(),
        })
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn a_change_from_a_version_that_has_moved_on_is_refused() {
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;
    server
        .patch(&format!("/ideas/{}", captured.id))
        .json(&RetitleIdeaRequest {
            title: "Someone else got here first".to_owned(),
        })
        .await
        .assert_status_ok();

    refused_as(
        server
            .patch(&format!("/ideas/{}", captured.id))
            .add_header("if-match", "\"1\"")
            .json(&RetitleIdeaRequest {
                title: "Working from a stale copy".to_owned(),
            })
            .await,
        StatusCode::PRECONDITION_FAILED,
        "outdated",
    );

    let response = server.get(&format!("/ideas/{}", captured.id)).await;
    assert_eq!(
        response.json::<IdeaDTO>().title,
        "Someone else got here first",
        "a refused change must leave the idea as it was"
    );
}

#[tokio::test]
async fn discarding_from_a_stale_version_is_refused() {
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;
    server
        .patch(&format!("/ideas/{}", captured.id))
        .json(&RetitleIdeaRequest {
            title: "The Silent Loom".to_owned(),
        })
        .await
        .assert_status_ok();

    refused_as(
        server
            .delete(&format!("/ideas/{}", captured.id))
            .add_header("if-match", "\"1\"")
            .await,
        StatusCode::PRECONDITION_FAILED,
        "outdated",
    );

    server
        .get(&format!("/ideas/{}", captured.id))
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn any_version_will_do_when_the_caller_says_so() {
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;
    server
        .patch(&format!("/ideas/{}", captured.id))
        .json(&RetitleIdeaRequest {
            title: "Moved on".to_owned(),
        })
        .await
        .assert_status_ok();

    server
        .patch(&format!("/ideas/{}", captured.id))
        .add_header("if-match", "*")
        .json(&RetitleIdeaRequest {
            title: "Whatever it says now".to_owned(),
        })
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn an_if_match_that_is_not_a_version_is_a_bad_request() {
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    refused_as(
        server
            .patch(&format!("/ideas/{}", captured.id))
            .add_header("if-match", "\"not-a-version\"")
            .json(&RetitleIdeaRequest {
                title: "Nowhere".to_owned(),
            })
            .await,
        StatusCode::BAD_REQUEST,
        "unreadable",
    );
}

#[tokio::test]
async fn a_version_that_never_existed_is_a_failed_precondition_not_a_missing_idea() {
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    refused_as(
        server
            .patch(&format!("/ideas/{}", captured.id))
            .add_header("if-match", "\"0\"")
            .json(&RetitleIdeaRequest {
                title: "Stale".to_owned(),
            })
            .await,
        StatusCode::PRECONDITION_FAILED,
        "outdated",
    );

    refused_as(
        server
            .patch(&format!("/ideas/{}", captured.id))
            .add_header("if-match", "\"99\"")
            .json(&RetitleIdeaRequest {
                title: "From the future".to_owned(),
            })
            .await,
        StatusCode::PRECONDITION_FAILED,
        "outdated",
    );
}

#[tokio::test]
async fn a_precondition_on_an_idea_nobody_captured_is_still_not_found() {
    let server = a_server();

    refused_as(
        server
            .patch(&format!("/ideas/{UNKNOWN_ID}"))
            .add_header("if-match", "\"1\"")
            .json(&RetitleIdeaRequest {
                title: "Nowhere".to_owned(),
            })
            .await,
        StatusCode::NOT_FOUND,
        "not-found",
    );
}

#[tokio::test]
async fn a_project_nobody_wrote_in_lists_nothing() {
    let server = a_server();

    let response = server.get("/ideas?project=project_empty").await;

    response.assert_status_ok();
    assert!(response.json::<Vec<IdeaDTO>>().is_empty());
}

#[tokio::test]
async fn captured_ideas_are_listed_for_their_project() {
    let server = a_server();
    let one = an_idea(&server, "The Loom").await;
    let other = an_idea(&server, "The Shuttle").await;

    let listed = server.listed("project_1").await;

    assert_eq!(listed.len(), 2);
    assert!(listed.iter().any(|idea| idea.id == one.id));
    assert!(listed.iter().any(|idea| idea.id == other.id));
}

#[tokio::test]
async fn listing_asks_for_a_project() {
    let server = a_server();

    server
        .get("/ideas")
        .await
        .assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_retitled_idea_is_listed_under_its_new_title() {
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    server
        .patch(&format!("/ideas/{}", captured.id))
        .json(&RetitleIdeaRequest {
            title: "The Silent Loom".to_owned(),
        })
        .await
        .assert_status_ok();

    let listed = server.listed("project_1").await;

    assert_eq!(listed[0].title, "The Silent Loom");
    assert_eq!(listed[0].version, 2, "the listing must not go stale");
}

#[tokio::test]
async fn a_discarded_idea_leaves_the_listing() {
    let server = a_server();
    let kept = an_idea(&server, "The Loom").await;
    let discarded = an_idea(&server, "A false start").await;

    server
        .delete(&format!("/ideas/{}", discarded.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    let listed = server.listed("project_1").await;

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, kept.id);
}

#[tokio::test]
async fn a_refused_change_leaves_the_listing_alone() {
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    refused_as(
        server
            .patch(&format!("/ideas/{}", captured.id))
            .add_header("if-match", "\"99\"")
            .json(&RetitleIdeaRequest {
                title: "Never happened".to_owned(),
            })
            .await,
        StatusCode::PRECONDITION_FAILED,
        "outdated",
    );

    let listed = server.listed("project_1").await;

    assert_eq!(
        listed[0].title, "The Loom",
        "a change that never landed must not reach the listing either"
    );
    assert_eq!(listed[0].version, 1);
}

#[tokio::test]
async fn an_idea_title_with_a_control_character_is_refused_as_such() {
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    let response = server
        .patch(&format!("/ideas/{}", captured.id))
        .json(&RetitleIdeaRequest {
            title: "The\u{7} Loom".to_owned(),
        })
        .await;

    refused_as(
        response,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::TITLE_CONTROL_CHARACTER,
    );
}
