use std::sync::Arc;

use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use passages_contract::{FRAGMENT, PassageDTO};
use passages_core::PassageService;
use passages_store::InMemoryPassageStore;
use time::{Duration, OffsetDateTime};
use yrs::{Doc, ReadTxn, StateVector, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim};

const UNKNOWN_ID: &str = "019a4f4a-0000-7000-8000-000000000000";

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
    let response = server.post("/passages").await;
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
        .absorb(&created.id, &a_paragraph("The loom stood silent."))
        .await
        .expect("should absorb");

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
