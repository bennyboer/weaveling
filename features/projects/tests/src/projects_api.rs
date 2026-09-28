use std::sync::Arc;

use axum::http::StatusCode;
use axum::http::header::ETAG;
use axum_test::TestServer;
use clock::FixedClock;
use projects_contract::{CreateProjectRequest, ProjectDTO, RenameProjectRequest};
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

fn named(name: &str) -> CreateProjectRequest {
    CreateProjectRequest {
        name: name.to_owned(),
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

    response.assert_status(StatusCode::BAD_REQUEST);
    assert!(
        response.text().contains("blank"),
        "body was {}",
        response.text()
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

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn fetching_a_malformed_id_answers_400() {
    let server = a_server();

    let response = server.get("/projects/weaveling").await;

    response.assert_status(StatusCode::BAD_REQUEST);
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

    response.assert_status(StatusCode::BAD_REQUEST);
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

    server
        .patch(&format!("/projects/{}", created.id))
        .add_header("if-match", "\"1\"")
        .json(&renamed("Twice"))
        .await
        .assert_status(StatusCode::PRECONDITION_FAILED);
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

    server
        .patch(&format!("/projects/{}", created.id))
        .json(&renamed("Too late"))
        .await
        .assert_status(StatusCode::CONFLICT);
}

#[tokio::test]
async fn deleting_an_unknown_project_answers_404() {
    let server = a_server();

    let response = server.delete(&format!("/projects/{UNKNOWN_ID}")).await;

    response.assert_status(StatusCode::NOT_FOUND);
}
