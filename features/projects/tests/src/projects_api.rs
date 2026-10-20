use std::sync::Arc;

use axum::http::StatusCode;
use axum::http::header::{CONTENT_TYPE, ETAG};
use axum_test::{TestResponse, TestServer};
use clock::FixedClock;
use projects_contract::{
    ChangeDetailsRequest, CreateProjectRequest, ProjectDTO, ProjectDetailsDTO,
    RenameProjectRequest, refused,
};
use serde_json::{Value, json};
use time::{Duration, OffsetDateTime};

use crate::wiring::{Wired, wired};

const UNKNOWN_ID: &str = "project_031VkO0hnpeQZUiAB7nDma";

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
    async fn listed(&self) -> Vec<ProjectDTO> {
        self.wired.settle().await;

        self.server.get("/projects").await.json()
    }
}

fn a_server() -> Serving {
    a_server_on(Arc::new(FixedClock::new(at(1_700_000_000))))
}

fn a_server_on(clock: Arc<FixedClock>) -> Serving {
    let wired = wired(clock);
    let server = TestServer::new(wired.routes.clone());

    Serving { server, wired }
}

fn refused_as(response: &TestResponse, status: StatusCode, kind: &str) -> Value {
    response.assert_status(status);
    assert_eq!(response.header(CONTENT_TYPE), "application/problem+json");
    let told: Value = response.json();
    assert_eq!(told["type"], kind, "{told}");
    assert_eq!(told["status"], status.as_u16(), "{told}");

    told
}

fn named(name: &str) -> CreateProjectRequest {
    CreateProjectRequest {
        name: name.to_owned(),
        language: None,
        quotation: None,
        description: String::new(),
    }
}

fn renamed(name: &str) -> RenameProjectRequest {
    RenameProjectRequest {
        name: name.to_owned(),
    }
}

async fn a_project_named(server: &Serving, name: &str) -> ProjectDTO {
    let response = server.post("/projects").json(&named(name)).await;
    response.assert_status(StatusCode::CREATED);

    response.json()
}

#[tokio::test]
async fn starting_a_project_answers_201_with_the_new_project() {
    let server = a_server();

    let response = server.post("/projects").json(&named("Tapestry")).await;

    response.assert_status(StatusCode::CREATED);
    let created: ProjectDTO = response.json();
    assert_eq!(created.name, "Tapestry");
    assert_eq!(created.version, 1);
    assert_eq!(created.created_at, "2023-11-14T22:13:20Z");
    assert_eq!(created.updated_at, created.created_at);
    assert!(!created.id.is_empty());
}

#[tokio::test]
async fn starting_a_project_trims_the_name() {
    let server = a_server();

    let response = server.post("/projects").json(&named("  Tapestry  ")).await;

    response.assert_status(StatusCode::CREATED);
    assert_eq!(response.json::<ProjectDTO>().name, "Tapestry");
}

#[tokio::test]
async fn starting_a_project_with_a_blank_name_answers_400() {
    let server = a_server();

    let response = server.post("/projects").json(&named("   ")).await;

    let told = refused_as(&response, StatusCode::BAD_REQUEST, refused::NAME_BLANK);
    assert_eq!(told["detail"], "project name must not be blank");
}

#[tokio::test]
async fn a_name_too_long_is_refused_telling_how_long_it_may_be() {
    let server = a_server();

    let response = server
        .post("/projects")
        .json(&named(&"a".repeat(201)))
        .await;

    let told = refused_as(&response, StatusCode::BAD_REQUEST, refused::NAME_TOO_LONG);
    assert_eq!(told["max"], 200);
    assert_eq!(told["actual"], 201);
}

#[tokio::test]
async fn a_name_with_a_control_character_is_refused_as_such() {
    let server = a_server();

    let response = server.post("/projects").json(&named("Tape\u{7}stry")).await;

    refused_as(
        &response,
        StatusCode::BAD_REQUEST,
        refused::NAME_CONTROL_CHARACTER,
    );
}

#[tokio::test]
async fn a_started_project_appears_in_the_listing() {
    let server = a_server();
    let created = a_project_named(&server, "Tapestry").await;

    assert_eq!(server.listed().await, vec![created]);
}

#[tokio::test]
async fn listing_an_empty_workspace_answers_an_empty_array() {
    let server = a_server();

    let response = server.get("/projects").await;

    response.assert_status(StatusCode::OK);
    assert!(response.json::<Vec<ProjectDTO>>().is_empty());
}

#[tokio::test]
async fn the_project_started_most_recently_is_listed_first() {
    let clock = Arc::new(FixedClock::new(at(1_700_000_000)));
    let server = a_server_on(clock.clone());
    a_project_named(&server, "First").await;
    clock.set(at(1_700_000_060));
    a_project_named(&server, "Second").await;

    let listed = server.listed().await;

    assert_eq!(
        listed
            .iter()
            .map(|project| project.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Second", "First"]
    );
}

#[tokio::test]
async fn a_started_project_can_be_fetched_by_id() {
    let server = a_server();
    let created = a_project_named(&server, "Tapestry").await;

    let response = server.get(&format!("/projects/{}", created.id)).await;

    response.assert_status(StatusCode::OK);
    assert_eq!(response.json::<ProjectDTO>(), created);
}

#[tokio::test]
async fn fetching_an_unknown_project_answers_404() {
    let server = a_server();

    let response = server.get(&format!("/projects/{UNKNOWN_ID}")).await;

    refused_as(&response, StatusCode::NOT_FOUND, "not-found");
}

#[tokio::test]
async fn fetching_a_malformed_id_answers_400() {
    let server = a_server();

    let response = server.get("/projects/weaveling").await;

    refused_as(&response, StatusCode::BAD_REQUEST, refused::ID_INVALID);
}

#[tokio::test]
async fn renaming_a_project_answers_the_updated_project() {
    let clock = Arc::new(FixedClock::new(at(1_700_000_000)));
    let server = a_server_on(clock.clone());
    let created = a_project_named(&server, "Working Title").await;

    clock.set(at(1_700_000_060));
    let response = server
        .patch(&format!("/projects/{}", created.id))
        .json(&renamed("The Weaver's Apprentice"))
        .await;

    response.assert_status(StatusCode::OK);
    let updated: ProjectDTO = response.json();
    assert_eq!(updated.name, "The Weaver's Apprentice");
    assert_eq!(updated.id, created.id);
    assert_eq!(updated.version, 2);
    assert_eq!(updated.created_at, created.created_at);
    assert_eq!(updated.updated_at, "2023-11-14T22:14:20Z");
}

#[tokio::test]
async fn renaming_an_unknown_project_answers_404() {
    let server = a_server();

    let response = server
        .patch(&format!("/projects/{UNKNOWN_ID}"))
        .json(&renamed("Renamed"))
        .await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn renaming_to_a_blank_name_answers_400() {
    let server = a_server();
    let created = a_project_named(&server, "Working Title").await;

    let response = server
        .patch(&format!("/projects/{}", created.id))
        .json(&renamed(""))
        .await;

    refused_as(&response, StatusCode::BAD_REQUEST, refused::NAME_BLANK);
}

#[tokio::test]
async fn a_project_reports_the_version_it_stands_at() {
    let server = a_server();

    let created = a_project_named(&server, "Tapestry").await;

    let response = server.get(&format!("/projects/{}", created.id)).await;
    assert_eq!(
        response
            .headers()
            .get(ETAG)
            .and_then(|tag| tag.to_str().ok()),
        Some("\"1\""),
        "an author cannot ask to change a version they were never told"
    );
}

#[tokio::test]
async fn renaming_from_the_version_the_author_holds_is_allowed() {
    let server = a_server();
    let created = a_project_named(&server, "Working Title").await;

    server
        .patch(&format!("/projects/{}", created.id))
        .add_header("if-match", "\"1\"")
        .json(&renamed("The Weaver's Apprentice"))
        .await
        .assert_status(StatusCode::OK);
}

#[tokio::test]
async fn renaming_from_a_stale_version_answers_412() {
    let server = a_server();
    let created = a_project_named(&server, "Working Title").await;
    server
        .patch(&format!("/projects/{}", created.id))
        .json(&renamed("Once"))
        .await
        .assert_status(StatusCode::OK);

    let response = server
        .patch(&format!("/projects/{}", created.id))
        .add_header("if-match", "\"1\"")
        .json(&renamed("Twice"))
        .await;

    refused_as(&response, StatusCode::PRECONDITION_FAILED, "outdated");
}

#[tokio::test]
async fn an_unreadable_version_asked_for_is_refused_as_unreadable() {
    let server = a_server();
    let created = a_project_named(&server, "Working Title").await;

    let response = server
        .patch(&format!("/projects/{}", created.id))
        .add_header("if-match", "\"seven\"")
        .json(&renamed("Twice"))
        .await;

    refused_as(&response, StatusCode::BAD_REQUEST, "unreadable");
}

#[tokio::test]
async fn deleting_a_project_answers_204_and_it_is_gone() {
    let server = a_server();
    let created = a_project_named(&server, "Tapestry").await;

    let response = server.delete(&format!("/projects/{}", created.id)).await;

    response.assert_status(StatusCode::NO_CONTENT);
    server
        .get(&format!("/projects/{}", created.id))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    assert!(server.listed().await.is_empty());
}

#[tokio::test]
async fn a_deleted_project_refuses_to_be_renamed() {
    let server = a_server();
    let created = a_project_named(&server, "Abandoned").await;
    server
        .delete(&format!("/projects/{}", created.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    let response = server
        .patch(&format!("/projects/{}", created.id))
        .json(&renamed("Too late"))
        .await;

    refused_as(&response, StatusCode::CONFLICT, refused::DELETED);
}

#[tokio::test]
async fn deleting_an_unknown_project_answers_404() {
    let server = a_server();

    let response = server.delete(&format!("/projects/{UNKNOWN_ID}")).await;

    response.assert_status(StatusCode::NOT_FOUND);
}

fn details(language: Option<&str>, description: &str) -> ChangeDetailsRequest {
    ChangeDetailsRequest {
        language: language.map(str::to_owned),
        quotation: None,
        description: description.to_owned(),
    }
}

fn quoted(language: &str, quotation: &str) -> ChangeDetailsRequest {
    ChangeDetailsRequest {
        quotation: Some(quotation.to_owned()),
        ..details(Some(language), "")
    }
}

#[tokio::test]
async fn a_project_can_be_started_with_its_language_and_a_description() {
    let server = a_server();

    let response = server
        .post("/projects")
        .json(&CreateProjectRequest {
            name: "Der Webstuhl".to_owned(),
            language: Some("de".to_owned()),
            quotation: Some("low-high".to_owned()),
            description: "A weaver's daughter, and a war.".to_owned(),
        })
        .await;

    response.assert_status(StatusCode::CREATED);
    let started: ProjectDetailsDTO = response.json();
    let found: ProjectDetailsDTO = server
        .get(&format!("/projects/{}", started.id))
        .await
        .json();
    assert_eq!(found.language.as_deref(), Some("de"));
    assert_eq!(found.quotation.as_deref(), Some("low-high"));
    assert_eq!(found.description, "A weaver's daughter, and a war.");
}

#[tokio::test]
async fn a_project_started_with_a_name_alone_has_no_language_yet() {
    let server = a_server();
    let started = a_project_named(&server, "Working Title").await;

    let found: ProjectDetailsDTO = server
        .get(&format!("/projects/{}", started.id))
        .await
        .json();

    assert_eq!(found.language, None);
    assert_eq!(
        found.quotation, None,
        "with no style chosen, the book's language decides its quotation marks"
    );
    assert_eq!(found.description, "");
}

#[tokio::test]
async fn a_project_s_language_and_description_can_be_changed() {
    let server = a_server();
    let started = a_project_named(&server, "Working Title").await;

    let response = server
        .put(&format!("/projects/{}/details", started.id))
        .json(&details(Some("EN-gb"), "  A crown of straw.  "))
        .await;

    response.assert_status(StatusCode::OK);
    let changed: ProjectDetailsDTO = response.json();
    assert_eq!(
        changed.language.as_deref(),
        Some("en-GB"),
        "a language is kept in the case `lang` expects"
    );
    assert_eq!(changed.description, "A crown of straw.");
    assert_eq!(changed.version, 2);
}

#[tokio::test]
async fn what_is_not_a_language_or_too_long_a_description_is_refused() {
    let server = a_server();
    let started = a_project_named(&server, "Working Title").await;

    for (asked, kind, told) in [
        (
            details(Some("deutsch"), ""),
            refused::LANGUAGE_NOT_A_TAG,
            json!({ "tag": "deutsch" }),
        ),
        (
            details(None, &"a".repeat(4_097)),
            refused::DESCRIPTION_TOO_LONG,
            json!({ "max": 4_096, "actual": 4_097 }),
        ),
        (
            details(None, "A crown\u{7} of straw."),
            refused::DESCRIPTION_CONTROL_CHARACTER,
            json!({}),
        ),
        (
            quoted("de", "guillemets"),
            refused::QUOTATION_UNKNOWN,
            json!({ "name": "guillemets" }),
        ),
    ] {
        let response = server
            .put(&format!("/projects/{}/details", started.id))
            .json(&asked)
            .await;

        let refusal = refused_as(&response, StatusCode::BAD_REQUEST, kind);
        for (name, value) in told.as_object().unwrap() {
            assert_eq!(&refusal[name], value, "{kind} tells {name}");
        }
    }
}

#[tokio::test]
async fn a_book_s_quotation_style_can_be_chosen_and_chosen_again() {
    let server = a_server();
    let started = a_project_named(&server, "Working Title").await;

    for style in [
        "pointing-in",
        "low-high",
        "curly",
        "spaced-out",
        "pointing-out",
    ] {
        let changed: ProjectDetailsDTO = server
            .put(&format!("/projects/{}/details", started.id))
            .json(&quoted("de", style))
            .await
            .json();

        assert_eq!(changed.quotation.as_deref(), Some(style));
    }
}

#[tokio::test]
async fn changing_the_details_of_an_unknown_project_answers_404() {
    let server = a_server();

    server
        .put(&format!("/projects/{UNKNOWN_ID}/details"))
        .json(&details(Some("de"), ""))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}
