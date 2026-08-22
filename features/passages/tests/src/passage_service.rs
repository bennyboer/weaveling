use std::sync::Arc;

use clock::FixedClock;
use passages_core::{FRAGMENT, PassageService, PassageServiceError, StoreError};
use passages_store::InMemoryPassageStore;
use time::{Duration, OffsetDateTime};
use yrs::{Doc, ReadTxn, StateVector, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn new_service() -> PassageService {
    new_service_with(Arc::new(FixedClock::new(at(1_000))))
}

fn new_service_with(clock: Arc<FixedClock>) -> PassageService {
    PassageService::new(Arc::new(InMemoryPassageStore::new()), clock)
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

#[tokio::test]
async fn a_new_passage_starts_empty_and_can_be_opened_again() {
    let service = new_service();

    let created = service.create().await.expect("should create");
    let opened = service
        .open(&created.id().to_string())
        .await
        .expect("should open");

    assert_eq!(opened.text(), "");
    assert_eq!(opened.id(), created.id());
}

#[tokio::test]
async fn a_passage_id_records_when_it_was_created() {
    let clock = Arc::new(FixedClock::new(at(1_700_000_000)));
    let service = new_service_with(clock);

    let passage = service.create().await.expect("should create");

    let (seconds, _) = passage
        .id()
        .as_uuid()
        .get_timestamp()
        .expect("a v7 id carries a timestamp")
        .to_unix();
    assert_eq!(seconds, 1_700_000_000);
}

#[tokio::test]
async fn two_passages_created_at_the_same_moment_are_still_distinct() {
    let service = new_service();

    let one = service.create().await.expect("should create");
    let other = service.create().await.expect("should create");

    assert_ne!(one.id(), other.id());
}

#[tokio::test]
async fn writing_prose_is_visible_when_the_passage_is_opened_again() {
    let service = new_service();
    let passage = service.create().await.expect("should create");
    let id = passage.id().to_string();

    service
        .absorb(&id, &a_paragraph("The loom stood silent."))
        .await
        .expect("should absorb");

    let reopened = service.open(&id).await.expect("should open");
    assert_eq!(reopened.text(), "The loom stood silent.");
}

#[tokio::test]
async fn a_malformed_id_is_rejected_before_the_store_is_touched() {
    let service = new_service();

    let error = service.open("weaveling").await.expect_err("should reject");

    assert!(
        matches!(error, PassageServiceError::InvalidId(_)),
        "expected InvalidId, got {error:?}"
    );
}

#[tokio::test]
async fn opening_a_passage_that_was_never_created_is_not_found() {
    let service = new_service();
    let never_created = service.create().await.expect("should create");
    service
        .delete(&never_created.id().to_string())
        .await
        .expect("should delete");

    let error = service
        .open(&never_created.id().to_string())
        .await
        .expect_err("should not find it");

    assert!(
        matches!(error, PassageServiceError::Store(StoreError::NotFound(_))),
        "expected NotFound, got {error:?}"
    );
}

#[tokio::test]
async fn a_deleted_passage_takes_its_prose_with_it() {
    let service = new_service();
    let passage = service.create().await.expect("should create");
    let id = passage.id().to_string();
    service
        .absorb(&id, &a_paragraph("The loom stood silent."))
        .await
        .expect("should absorb");

    service.delete(&id).await.expect("should delete");

    let error = service.open(&id).await.expect_err("should be gone");
    assert!(matches!(
        error,
        PassageServiceError::Store(StoreError::NotFound(_))
    ));
}

#[tokio::test]
async fn an_unusable_update_leaves_the_prose_as_it_was() {
    let service = new_service();
    let passage = service.create().await.expect("should create");
    let id = passage.id().to_string();
    service
        .absorb(&id, &a_paragraph("The loom stood silent."))
        .await
        .expect("should absorb");

    let error = service
        .absorb(&id, &[255, 255, 255, 255])
        .await
        .expect_err("garbage should be refused");

    assert!(matches!(
        error,
        PassageServiceError::Store(StoreError::Unusable(_))
    ));
    let reopened = service.open(&id).await.expect("should still open");
    assert_eq!(reopened.text(), "The loom stood silent.");
}

#[tokio::test]
async fn a_passage_written_by_two_authors_holds_both_contributions() {
    let service = new_service();
    let passage = service.create().await.expect("should create");
    let id = passage.id().to_string();

    service
        .absorb(&id, &a_paragraph("Ada wrote this."))
        .await
        .expect("ada should write");
    service
        .absorb(&id, &a_paragraph("Bo wrote this."))
        .await
        .expect("bo should write");

    let reopened = service.open(&id).await.expect("should open");
    assert!(reopened.text().contains("Ada wrote this."));
    assert!(reopened.text().contains("Bo wrote this."));
}
