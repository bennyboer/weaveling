use std::sync::Arc;

use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use scenes_contract::{
    CreateSceneRequest, FRAGMENT, LinkIdeaRequest, RetitleSceneRequest, SceneDTO,
};
use scenes_core::SceneService;
use scenes_store::InMemorySceneStore;
use time::{Duration, OffsetDateTime};
use yrs::{Doc, ReadTxn, StateVector, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim};

const UNKNOWN_ID: &str = "scene_031VkO0hnpeQZUiAB7nDma";

const A_PROJECT: &str = "project_1";

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn a_service() -> SceneService {
    SceneService::new(
        Arc::new(InMemorySceneStore::new()),
        Arc::new(FixedClock::new(at(1_700_000_000))),
    )
}

fn new_server_with(service: SceneService) -> TestServer {
    TestServer::new(scenes_rest::router(service))
}

fn a_paragraph(saying: &str) -> Vec<u8> {
    let doc = Doc::new();
    let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
    {
        let mut txn = doc.transact_mut();
        let paragraph = fragment.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
        paragraph.insert(&mut txn, 0, XmlTextPrelim::new(saying));
    }

    doc.transact()
        .encode_state_as_update_v1(&StateVector::default())
}

async fn a_scene(server: &TestServer) -> SceneDTO {
    let response = server
        .post("/scenes")
        .json(&CreateSceneRequest {
            project: A_PROJECT.to_owned(),
        })
        .await;
    response.assert_status(StatusCode::CREATED);

    response.json()
}

#[tokio::test]
async fn a_created_scene_is_reported_with_an_id_and_no_prose() {
    let server = new_server_with(a_service());

    let created = a_scene(&server).await;

    assert!(!created.id.is_empty(), "a scene must be given an id");
    assert_eq!(created.text, "", "a new scene holds no prose");
}

#[tokio::test]
async fn two_created_scenes_are_distinct() {
    let server = new_server_with(a_service());

    let one = a_scene(&server).await;
    let other = a_scene(&server).await;

    assert_ne!(one.id, other.id);
}

#[tokio::test]
async fn a_scene_can_be_read_back_by_its_id() {
    let server = new_server_with(a_service());
    let created = a_scene(&server).await;

    let response = server.get(&format!("/scenes/{}", created.id)).await;

    response.assert_status_ok();
    assert_eq!(response.json::<SceneDTO>(), created);
}

#[tokio::test]
async fn the_read_model_reports_the_prose_that_was_written() {
    let service = a_service();
    let server = new_server_with(service.clone());
    let created = a_scene(&server).await;
    service
        .apply(&created.id, &a_paragraph("The loom stood silent."))
        .await
        .expect("should apply");

    let response = server.get(&format!("/scenes/{}", created.id)).await;

    response.assert_status_ok();
    assert_eq!(
        response.json::<SceneDTO>().text,
        "The loom stood silent.",
        "the projection must show what the peers see"
    );
}

#[tokio::test]
async fn a_malformed_id_is_a_bad_request() {
    let server = new_server_with(a_service());

    let response = server.get("/scenes/weaveling").await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn an_unknown_scene_is_not_found() {
    let server = new_server_with(a_service());

    let response = server.get(&format!("/scenes/{UNKNOWN_ID}")).await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_deleted_scene_is_gone() {
    let server = new_server_with(a_service());
    let created = a_scene(&server).await;

    server
        .delete(&format!("/scenes/{}", created.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    server
        .get(&format!("/scenes/{}", created.id))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn deleting_an_unknown_scene_is_not_found() {
    let server = new_server_with(a_service());

    let response = server.delete(&format!("/scenes/{UNKNOWN_ID}")).await;

    response.assert_status(StatusCode::NOT_FOUND);
}

async fn retitled(server: &TestServer, id: &str, title: &str) -> axum_test::TestResponse {
    server
        .patch(&format!("/scenes/{id}"))
        .json(&RetitleSceneRequest {
            title: title.to_owned(),
        })
        .await
}

#[tokio::test]
async fn a_new_scene_is_reported_untitled() {
    let server = new_server_with(a_service());

    let created = a_scene(&server).await;

    assert_eq!(
        created.title, "",
        "an untitled scene is named by its opening words"
    );
}

#[tokio::test]
async fn a_scene_can_be_retitled_and_read_back() {
    let server = new_server_with(a_service());
    let created = a_scene(&server).await;

    let response = retitled(&server, &created.id, "  The loom  ").await;

    response.assert_status_ok();
    assert_eq!(response.json::<SceneDTO>().title, "The loom");
    let found: SceneDTO = server.get(&format!("/scenes/{}", created.id)).await.json();
    assert_eq!(found.title, "The loom");
}

#[tokio::test]
async fn a_title_the_domain_refuses_is_unprocessable() {
    let server = new_server_with(a_service());
    let created = a_scene(&server).await;

    let response = retitled(&server, &created.id, "The\u{7}loom").await;

    response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn retitling_a_scene_nobody_created_is_not_found() {
    let server = new_server_with(a_service());

    let response = retitled(&server, UNKNOWN_ID, "Nowhere").await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_listing_carries_each_scene_s_title() {
    let server = new_server_with(a_service());
    let created = a_scene(&server).await;
    retitled(&server, &created.id, "The loom")
        .await
        .assert_status_ok();

    let listed: Vec<SceneDTO> = server
        .get(&format!("/scenes?project={A_PROJECT}"))
        .await
        .json();

    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0].title, "The loom",
        "the outline names its leaves from this listing"
    );
}

async fn linked(server: &TestServer, id: &str, idea: &str) -> axum_test::TestResponse {
    server
        .post(&format!("/scenes/{id}/ideas"))
        .json(&LinkIdeaRequest {
            idea: idea.to_owned(),
        })
        .await
}

#[tokio::test]
async fn a_new_scene_has_no_linked_ideas() {
    let server = new_server_with(a_service());

    let created = a_scene(&server).await;

    assert!(created.ideas.is_empty());
}

#[tokio::test]
async fn an_idea_can_be_linked_and_unlinked() {
    let server = new_server_with(a_service());
    let created = a_scene(&server).await;

    let after_linking = linked(&server, &created.id, "idea_1").await;
    after_linking.assert_status_ok();
    assert_eq!(after_linking.json::<SceneDTO>().ideas, vec!["idea_1"]);

    let after_unlinking = server
        .delete(&format!("/scenes/{}/ideas/idea_1", created.id))
        .await;
    after_unlinking.assert_status_ok();
    assert!(after_unlinking.json::<SceneDTO>().ideas.is_empty());
}

#[tokio::test]
async fn linking_to_a_scene_nobody_created_is_not_found() {
    let server = new_server_with(a_service());

    linked(&server, UNKNOWN_ID, "idea_1")
        .await
        .assert_status(StatusCode::NOT_FOUND);
    server
        .delete(&format!("/scenes/{UNKNOWN_ID}/ideas/idea_1"))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}
