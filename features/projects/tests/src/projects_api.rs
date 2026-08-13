use std::sync::Arc;

use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use projects_contract::{CreateProjectRequest, ProjectDTO, RenameProjectRequest};
use projects_core::ProjectService;
use projects_store::InMemoryProjectStore;
use time::{Duration, OffsetDateTime};

const UNKNOWN_ID: &str = "019a4f4a-0000-7000-8000-000000000000";

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn new_server() -> TestServer {
    new_server_with(Arc::new(FixedClock::new(at(1_700_000_000))))
}

fn new_server_with(clock: Arc<FixedClock>) -> TestServer {
    let store = Arc::new(InMemoryProjectStore::new());

    TestServer::new(projects_rest::router(ProjectService::new(store, clock)))
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

async fn a_project_named(server: &TestServer, name: &str) -> ProjectDTO {
    let response = server.post("/projects").json(&named(name)).await;
    response.assert_status(StatusCode::CREATED);

    response.json()
}

#[tokio::test]
async fn creating_a_project_answers_201_with_the_new_project() {
    let server = new_server();

    let response = server.post("/projects").json(&named("Tapestry")).await;

    response.assert_status(StatusCode::CREATED);
    let created: ProjectDTO = response.json();
    assert_eq!(created.name, "Tapestry");
    assert_eq!(created.created_at, "2023-11-14T22:13:20Z");
    assert_eq!(created.updated_at, created.created_at);
    assert!(!created.id.is_empty());
}

#[tokio::test]
async fn creating_a_project_trims_the_name() {
    let server = new_server();

    let response = server.post("/projects").json(&named("  Tapestry  ")).await;

    response.assert_status(StatusCode::CREATED);
    assert_eq!(response.json::<ProjectDTO>().name, "Tapestry");
}

#[tokio::test]
async fn creating_a_project_with_a_blank_name_answers_400() {
    let server = new_server();

    let response = server.post("/projects").json(&named("   ")).await;

    response.assert_status(StatusCode::BAD_REQUEST);
    assert!(
        response.text().contains("blank"),
        "body was {}",
        response.text()
    );
}

#[tokio::test]
async fn a_created_project_appears_in_the_listing() {
    let server = new_server();
    let created = a_project_named(&server, "Tapestry").await;

    let response = server.get("/projects").await;

    response.assert_status(StatusCode::OK);
    assert_eq!(response.json::<Vec<ProjectDTO>>(), vec![created]);
}

#[tokio::test]
async fn listing_an_empty_workspace_answers_an_empty_array() {
    let server = new_server();

    let response = server.get("/projects").await;

    response.assert_status(StatusCode::OK);
    assert!(response.json::<Vec<ProjectDTO>>().is_empty());
}

#[tokio::test]
async fn a_created_project_can_be_fetched_by_id() {
    let server = new_server();
    let created = a_project_named(&server, "Tapestry").await;

    let response = server.get(&format!("/projects/{}", created.id)).await;

    response.assert_status(StatusCode::OK);
    assert_eq!(response.json::<ProjectDTO>(), created);
}

#[tokio::test]
async fn fetching_an_unknown_project_answers_404() {
    let server = new_server();

    let response = server.get(&format!("/projects/{UNKNOWN_ID}")).await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn fetching_a_malformed_id_answers_400() {
    let server = new_server();

    let response = server.get("/projects/weaveling").await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn renaming_a_project_answers_the_updated_project() {
    let clock = Arc::new(FixedClock::new(at(1_700_000_000)));
    let server = new_server_with(clock.clone());
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
    assert_eq!(updated.created_at, created.created_at);
    assert_eq!(updated.updated_at, "2023-11-14T22:14:20Z");
}

#[tokio::test]
async fn renaming_an_unknown_project_answers_404() {
    let server = new_server();

    let response = server
        .patch(&format!("/projects/{UNKNOWN_ID}"))
        .json(&renamed("Renamed"))
        .await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn renaming_to_a_blank_name_answers_400() {
    let server = new_server();
    let created = a_project_named(&server, "Working Title").await;

    let response = server
        .patch(&format!("/projects/{}", created.id))
        .json(&renamed(""))
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn deleting_a_project_answers_204_and_it_is_gone() {
    let server = new_server();
    let created = a_project_named(&server, "Tapestry").await;

    let response = server.delete(&format!("/projects/{}", created.id)).await;

    response.assert_status(StatusCode::NO_CONTENT);
    server
        .get(&format!("/projects/{}", created.id))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn deleting_an_unknown_project_answers_404() {
    let server = new_server();

    let response = server.delete(&format!("/projects/{UNKNOWN_ID}")).await;

    response.assert_status(StatusCode::NOT_FOUND);
}
