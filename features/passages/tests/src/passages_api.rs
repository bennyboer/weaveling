use std::sync::Arc;

use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use passages_contract::{
    CreatePassageRequest, FRAGMENT, LinkIdeaRequest, PassageDTO, RetitlePassageRequest,
};
use passages_core::PassageService;
use passages_store::InMemoryPassageStore;
use time::{Duration, OffsetDateTime};
use yrs::{Doc, ReadTxn, StateVector, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim};

const UNKNOWN_ID: &str = "passage_031VkO0hnpeQZUiAB7nDma";

const A_PROJECT: &str = "project_1";

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn a_service() -> PassageService {
    PassageService::new(
        Arc::new(InMemoryPassageStore::new()),
        Arc::new(FixedClock::new(at(1_700_000_000))),
    )
}

fn new_server_with(service: PassageService) -> TestServer {
    TestServer::new(passages_rest::router(service))
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

async fn a_passage(server: &TestServer) -> PassageDTO {
    let response = server
        .post("/passages")
        .json(&CreatePassageRequest {
            project: A_PROJECT.to_owned(),
        })
        .await;
    response.assert_status(StatusCode::CREATED);

    response.json()
}

#[tokio::test]
async fn a_created_passage_is_reported_with_an_id_and_no_prose() {
    let server = new_server_with(a_service());

    let created = a_passage(&server).await;

    assert!(!created.id.is_empty(), "a passage must be given an id");
    assert_eq!(created.text, "", "a new passage holds no prose");
}

#[tokio::test]
async fn two_created_passages_are_distinct() {
    let server = new_server_with(a_service());

    let one = a_passage(&server).await;
    let other = a_passage(&server).await;

    assert_ne!(one.id, other.id);
}

#[tokio::test]
async fn a_passage_can_be_read_back_by_its_id() {
    let server = new_server_with(a_service());
    let created = a_passage(&server).await;

    let response = server.get(&format!("/passages/{}", created.id)).await;

    response.assert_status_ok();
    assert_eq!(response.json::<PassageDTO>(), created);
}

#[tokio::test]
async fn the_read_model_reports_the_prose_that_was_written() {
    let service = a_service();
    let server = new_server_with(service.clone());
    let created = a_passage(&server).await;
    service
        .apply(&created.id, &a_paragraph("The loom stood silent."))
        .await
        .expect("should apply");

    let response = server.get(&format!("/passages/{}", created.id)).await;

    response.assert_status_ok();
    assert_eq!(
        response.json::<PassageDTO>().text,
        "The loom stood silent.",
        "the projection must show what the peers see"
    );
}

#[tokio::test]
async fn a_malformed_id_is_a_bad_request() {
    let server = new_server_with(a_service());

    let response = server.get("/passages/weaveling").await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn an_unknown_passage_is_not_found() {
    let server = new_server_with(a_service());

    let response = server.get(&format!("/passages/{UNKNOWN_ID}")).await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_deleted_passage_is_gone() {
    let server = new_server_with(a_service());
    let created = a_passage(&server).await;

    server
        .delete(&format!("/passages/{}", created.id))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    server
        .get(&format!("/passages/{}", created.id))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn deleting_an_unknown_passage_is_not_found() {
    let server = new_server_with(a_service());

    let response = server.delete(&format!("/passages/{UNKNOWN_ID}")).await;

    response.assert_status(StatusCode::NOT_FOUND);
}

async fn retitled(server: &TestServer, id: &str, title: &str) -> axum_test::TestResponse {
    server
        .patch(&format!("/passages/{id}"))
        .json(&RetitlePassageRequest {
            title: title.to_owned(),
        })
        .await
}

#[tokio::test]
async fn a_new_passage_is_reported_untitled() {
    let server = new_server_with(a_service());

    let created = a_passage(&server).await;

    assert_eq!(
        created.title, "",
        "an untitled passage is named by its opening words"
    );
}

#[tokio::test]
async fn a_passage_can_be_retitled_and_read_back() {
    let server = new_server_with(a_service());
    let created = a_passage(&server).await;

    let response = retitled(&server, &created.id, "  The loom  ").await;

    response.assert_status_ok();
    assert_eq!(response.json::<PassageDTO>().title, "The loom");
    let found: PassageDTO = server
        .get(&format!("/passages/{}", created.id))
        .await
        .json();
    assert_eq!(found.title, "The loom");
}

#[tokio::test]
async fn a_title_the_domain_refuses_is_unprocessable() {
    let server = new_server_with(a_service());
    let created = a_passage(&server).await;

    let response = retitled(&server, &created.id, "The\u{7}loom").await;

    response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn retitling_a_passage_nobody_created_is_not_found() {
    let server = new_server_with(a_service());

    let response = retitled(&server, UNKNOWN_ID, "Nowhere").await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_listing_carries_each_passage_s_title() {
    let server = new_server_with(a_service());
    let created = a_passage(&server).await;
    retitled(&server, &created.id, "The loom")
        .await
        .assert_status_ok();

    let listed: Vec<PassageDTO> = server
        .get(&format!("/passages?project={A_PROJECT}"))
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
        .post(&format!("/passages/{id}/ideas"))
        .json(&LinkIdeaRequest {
            idea: idea.to_owned(),
        })
        .await
}

#[tokio::test]
async fn a_new_passage_has_no_linked_ideas() {
    let server = new_server_with(a_service());

    let created = a_passage(&server).await;

    assert!(created.ideas.is_empty());
}

#[tokio::test]
async fn an_idea_can_be_linked_and_unlinked() {
    let server = new_server_with(a_service());
    let created = a_passage(&server).await;

    let after_linking = linked(&server, &created.id, "idea_1").await;
    after_linking.assert_status_ok();
    assert_eq!(after_linking.json::<PassageDTO>().ideas, vec!["idea_1"]);

    let after_unlinking = server
        .delete(&format!("/passages/{}/ideas/idea_1", created.id))
        .await;
    after_unlinking.assert_status_ok();
    assert!(after_unlinking.json::<PassageDTO>().ideas.is_empty());
}

#[tokio::test]
async fn linking_to_a_passage_nobody_created_is_not_found() {
    let server = new_server_with(a_service());

    linked(&server, UNKNOWN_ID, "idea_1")
        .await
        .assert_status(StatusCode::NOT_FOUND);
    server
        .delete(&format!("/passages/{UNKNOWN_ID}/ideas/idea_1"))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}
