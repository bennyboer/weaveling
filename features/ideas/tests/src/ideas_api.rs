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

const UNKNOWN_ID: &str = "idea_031VkO0hnpeQZUiAB7nDma";

fn refused_as(response: TestResponse, status: StatusCode, kind: &str) -> Value {
    response.assert_status(status);
    assert_eq!(response.header(CONTENT_TYPE), "application/problem+json");
    let told: Value = response.json();
    assert_eq!(told["type"], kind, "{told}");
    assert_eq!(told["status"], status.as_u16(), "{told}");

    told
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
    let wired = wiring::wired(Arc::new(FixedClock::new(clock::at(1_700_000_000))));
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
    // given: a server
    let server = a_server();

    // when: an idea is captured
    let captured = an_idea(&server, "The Loom").await;

    // then: its id says it is an idea
    assert!(
        captured.id.starts_with("idea_"),
        "an id should say what it is: {}",
        captured.id
    );
    // and: it comes back as it was captured
    assert_eq!(captured.title, "The Loom");
    assert_eq!(captured.project, "project_1");
}

#[tokio::test]
async fn an_idea_is_captured_by_whoever_the_request_says_acts() {
    // given: a server whose requests say an account acts
    let acting = Agent::Account(AccountId::from("account_7"));
    let server = a_server_acting_as(acting.clone());

    // when: an idea is captured
    let captured = an_idea(&server, "The Loom").await;

    // then: its capture is recorded as made by that account
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
    // given: a server whose requests say nobody acts
    let wired = wiring::wired(Arc::new(FixedClock::new(clock::at(1_700_000_000))));
    let server = TestServer::new(wired.routes.clone());

    // expect: capturing an idea is not served
    server
        .post("/ideas")
        .json(&a_capture("The Loom"))
        .await
        .assert_status(StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn an_idea_may_be_captured_with_no_title() {
    // given: a server
    let server = a_server();

    // when: an idea is captured with no title
    let captured = an_idea(&server, "").await;

    // then: it comes back untitled
    assert_eq!(captured.title, "");
}

#[tokio::test]
async fn two_captured_ideas_are_distinct() {
    // given: a server
    let server = a_server();

    // when: two ideas of the same title are captured
    let one = an_idea(&server, "The Loom").await;
    let other = an_idea(&server, "The Loom").await;

    // then: their ids differ
    assert_ne!(one.id, other.id);
}

#[tokio::test]
async fn a_captured_idea_can_be_fetched_again() {
    // given: a captured idea
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    // when: it is fetched
    let response = server.get(&format!("/ideas/{}", captured.id)).await;

    // then: it comes back as it was captured
    response.assert_status_ok();
    assert_eq!(response.json::<IdeaDTO>(), captured);
}

#[tokio::test]
async fn an_idea_nobody_captured_is_not_found() {
    // given: a server
    let server = a_server();

    // then: fetching an idea nobody captured is not found
    refused_as(
        server.get(&format!("/ideas/{UNKNOWN_ID}")).await,
        StatusCode::NOT_FOUND,
        "not-found",
    );
}

#[tokio::test]
async fn an_id_of_another_kind_is_a_bad_request() {
    // given: a server
    let server = a_server();

    // then: fetching by an id of another kind is a bad request
    refused_as(
        server.get("/ideas/scene_031VkO0hnpeQZUiAB7nDma").await,
        StatusCode::BAD_REQUEST,
        refused::ID_INVALID,
    );
}

#[tokio::test]
async fn an_id_without_its_prefix_is_a_bad_request() {
    // given: a server
    let server = a_server();

    // then: fetching by an id without its prefix is a bad request
    refused_as(
        server.get("/ideas/031VkO0hnpeQZUiAB7nDma").await,
        StatusCode::BAD_REQUEST,
        refused::ID_INVALID,
    );
}

#[tokio::test]
async fn an_idea_can_be_retitled() {
    // given: a captured idea
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    // when: it is retitled
    let response = server
        .patch(&format!("/ideas/{}", captured.id))
        .json(&RetitleIdeaRequest {
            title: "The Silent Loom".to_owned(),
        })
        .await;

    // then: it comes back under its new title
    response.assert_status_ok();
    assert_eq!(response.json::<IdeaDTO>().title, "The Silent Loom");
}

#[tokio::test]
async fn a_title_the_domain_refuses_is_a_bad_request() {
    // given: a captured idea
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    // when: it is retitled past the title's limit
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
    // then: the refusal tells the limit and how far it was passed
    assert_eq!(told["max"], IdeaTitle::MAX_CHARS);
    assert_eq!(told["actual"], IdeaTitle::MAX_CHARS + 1);
}

#[tokio::test]
async fn a_discarded_idea_is_gone_from_the_authors_point_of_view() {
    // given: a captured idea
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    // when: it is discarded
    server
        .delete(&format!("/ideas/{}", captured.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: retitling it is a conflict
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
    // given: a server
    let server = a_server();

    // then: discarding an idea nobody captured is not found
    refused_as(
        server.delete(&format!("/ideas/{UNKNOWN_ID}")).await,
        StatusCode::NOT_FOUND,
        "not-found",
    );
}

#[tokio::test]
async fn an_idea_reports_the_version_it_stands_at() {
    // given: a server
    let server = a_server();

    // when: an idea is captured
    let captured = an_idea(&server, "The Loom").await;

    // then: it stands at its first version
    assert_eq!(
        captured.version, 1,
        "a captured idea stands at its first version"
    );
}

#[tokio::test]
async fn every_response_carries_the_version_as_an_etag() {
    // given: a captured idea
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    // when: it is fetched
    let response = server.get(&format!("/ideas/{}", captured.id)).await;

    // then: its version comes as an etag
    assert_eq!(
        response.header("etag").to_str().expect("an ascii etag"),
        "\"1\"",
        "a caller needs somewhere to read the version it is looking at"
    );
}

#[tokio::test]
async fn a_change_moves_the_version_on() {
    // given: a captured idea
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    // when: it is retitled
    let response = server
        .patch(&format!("/ideas/{}", captured.id))
        .json(&RetitleIdeaRequest {
            title: "The Silent Loom".to_owned(),
        })
        .await;

    // then: it stands at its next version
    assert_eq!(response.json::<IdeaDTO>().version, 2);
}

#[tokio::test]
async fn a_change_from_the_version_the_caller_saw_is_accepted() {
    // given: a captured idea
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    // expect: retitling it from the version it stands at is accepted
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
    // given: an idea retitled since it was captured
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;
    server
        .patch(&format!("/ideas/{}", captured.id))
        .json(&RetitleIdeaRequest {
            title: "Someone else got here first".to_owned(),
        })
        .await
        .assert_status_ok();

    // when: it is retitled from its first version
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

    // then: the change is refused and the idea stays as it was
    let response = server.get(&format!("/ideas/{}", captured.id)).await;
    assert_eq!(
        response.json::<IdeaDTO>().title,
        "Someone else got here first",
        "a refused change must leave the idea as it was"
    );
}

#[tokio::test]
async fn discarding_from_a_stale_version_is_refused() {
    // given: an idea retitled since it was captured
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;
    server
        .patch(&format!("/ideas/{}", captured.id))
        .json(&RetitleIdeaRequest {
            title: "The Silent Loom".to_owned(),
        })
        .await
        .assert_status_ok();

    // when: it is discarded from its first version
    refused_as(
        server
            .delete(&format!("/ideas/{}", captured.id))
            .add_header("if-match", "\"1\"")
            .await,
        StatusCode::PRECONDITION_FAILED,
        "outdated",
    );

    // then: the discarding is refused and the idea is still there
    server
        .get(&format!("/ideas/{}", captured.id))
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn any_version_will_do_when_the_caller_says_so() {
    // given: an idea retitled since it was captured
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;
    server
        .patch(&format!("/ideas/{}", captured.id))
        .json(&RetitleIdeaRequest {
            title: "Moved on".to_owned(),
        })
        .await
        .assert_status_ok();

    // then: retitling it from any version is accepted
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
    // given: a captured idea
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    // then: retitling it from something that is not a version is a bad request
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
    // given: a captured idea
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    // then: retitling it from a version before its first is a failed precondition
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

    // and: from a version it never reached
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
    // given: a server
    let server = a_server();

    // then: retitling an idea nobody captured from a version is not found
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
    // given: a server
    let server = a_server();

    // when: a project nobody wrote in is listed
    let response = server.get("/ideas?project=project_empty").await;

    // then: nothing is listed
    response.assert_status_ok();
    assert!(response.json::<Vec<IdeaDTO>>().is_empty());
}

#[tokio::test]
async fn captured_ideas_are_listed_for_their_project() {
    // given: two ideas captured in a project
    let server = a_server();
    let one = an_idea(&server, "The Loom").await;
    let other = an_idea(&server, "The Shuttle").await;

    // when: the project is listed
    let listed = server.listed("project_1").await;

    // then: both are listed
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().any(|idea| idea.id == one.id));
    assert!(listed.iter().any(|idea| idea.id == other.id));
}

#[tokio::test]
async fn listing_asks_for_a_project() {
    // given: a server
    let server = a_server();

    // expect: listing without a project is a bad request
    server
        .get("/ideas")
        .await
        .assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_retitled_idea_is_listed_under_its_new_title() {
    // given: a captured idea
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    // when: it is retitled
    server
        .patch(&format!("/ideas/{}", captured.id))
        .json(&RetitleIdeaRequest {
            title: "The Silent Loom".to_owned(),
        })
        .await
        .assert_status_ok();

    // then: its project lists it under its new title and version
    let listed = server.listed("project_1").await;

    assert_eq!(listed[0].title, "The Silent Loom");
    assert_eq!(listed[0].version, 2, "the listing must not go stale");
}

#[tokio::test]
async fn a_discarded_idea_leaves_the_listing() {
    // given: two captured ideas
    let server = a_server();
    let kept = an_idea(&server, "The Loom").await;
    let discarded = an_idea(&server, "A false start").await;

    // when: one of them is discarded
    server
        .delete(&format!("/ideas/{}", discarded.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: only the other is listed
    let listed = server.listed("project_1").await;

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, kept.id);
}

#[tokio::test]
async fn a_refused_change_leaves_the_listing_alone() {
    // given: a captured idea
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    // when: a retitling of it is refused
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

    // then: it is listed as it was
    let listed = server.listed("project_1").await;

    assert_eq!(
        listed[0].title, "The Loom",
        "a change that never landed must not reach the listing either"
    );
    assert_eq!(listed[0].version, 1);
}

#[tokio::test]
async fn an_idea_title_with_a_control_character_is_refused_as_such() {
    // given: a captured idea
    let server = a_server();
    let captured = an_idea(&server, "The Loom").await;

    // when: it is retitled to a title with a control character
    let response = server
        .patch(&format!("/ideas/{}", captured.id))
        .json(&RetitleIdeaRequest {
            title: "The\u{7} Loom".to_owned(),
        })
        .await;

    // then: the title is refused for its control character
    refused_as(
        response,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::TITLE_CONTROL_CHARACTER,
    );
}
