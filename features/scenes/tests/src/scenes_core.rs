use std::sync::Arc;

use authorizing::AllowingEverything;
use clock::FixedClock;
use eventsourcing::Agent;
use scenes_catalog::{InMemoryQuotationRestylingTasks, InMemorySceneCatalog};
use scenes_core::{FRAGMENT, ScenesCore, ScenesError, StoreError};
use scenes_store::InMemorySceneStore;
use time::{Duration, OffsetDateTime};
use yrs::{Doc, ReadTxn, StateVector, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim};

const A_PROJECT: &str = "project_1";

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn new_core() -> ScenesCore {
    new_core_with(Arc::new(FixedClock::new(at(1_000))))
}

fn new_core_with(clock: Arc<FixedClock>) -> ScenesCore {
    ScenesCore::new(
        Arc::new(InMemorySceneStore::new()),
        Arc::new(InMemorySceneCatalog::new()),
        Arc::new(InMemoryQuotationRestylingTasks::new()),
        Arc::new(AllowingEverything),
        clock,
    )
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
async fn a_new_scene_starts_empty_and_can_be_opened_again() {
    let scenes = new_core();

    let created = scenes
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");
    let opened = scenes
        .open(&created.id().to_string())
        .await
        .expect("should open");

    assert_eq!(opened.text(), "");
    assert_eq!(opened.id(), created.id());
}

#[tokio::test]
async fn a_scene_id_records_when_it_was_created() {
    let clock = Arc::new(FixedClock::new(at(1_700_000_000)));
    let scenes = new_core_with(clock);

    let scene = scenes
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");

    let (seconds, _) = scene
        .id()
        .as_uuid()
        .get_timestamp()
        .expect("a v7 id carries a timestamp")
        .to_unix();
    assert_eq!(seconds, 1_700_000_000);
}

#[tokio::test]
async fn two_scenes_created_at_the_same_moment_are_still_distinct() {
    let scenes = new_core();

    let one = scenes
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");
    let other = scenes
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");

    assert_ne!(one.id(), other.id());
}

#[tokio::test]
async fn writing_prose_is_visible_when_the_scene_is_opened_again() {
    let scenes = new_core();
    let scene = scenes
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");
    let id = scene.id().to_string();

    scenes
        .apply(&id, &a_paragraph("The loom stood silent."))
        .await
        .expect("should apply");

    let reopened = scenes.open(&id).await.expect("should open");
    assert_eq!(reopened.text(), "The loom stood silent.");
}

#[tokio::test]
async fn a_malformed_id_is_rejected_before_the_store_is_touched() {
    let scenes = new_core();

    let error = scenes.open("weaveling").await.expect_err("should reject");

    assert!(
        matches!(error, ScenesError::InvalidId(_)),
        "expected InvalidId, got {error:?}"
    );
}

#[tokio::test]
async fn opening_a_scene_that_was_never_created_is_not_found() {
    let scenes = new_core();
    let never_created = scenes
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");
    scenes
        .delete(&never_created.id().to_string(), &Agent::Local)
        .await
        .expect("should delete");

    let error = scenes
        .open(&never_created.id().to_string())
        .await
        .expect_err("should not find it");

    assert!(
        matches!(error, ScenesError::Store(StoreError::NotFound(_))),
        "expected NotFound, got {error:?}"
    );
}

#[tokio::test]
async fn a_deleted_scene_takes_its_prose_with_it() {
    let scenes = new_core();
    let scene = scenes
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");
    let id = scene.id().to_string();
    scenes
        .apply(&id, &a_paragraph("The loom stood silent."))
        .await
        .expect("should apply");

    scenes
        .delete(&id, &Agent::Local)
        .await
        .expect("should delete");

    let error = scenes.open(&id).await.expect_err("should be gone");
    assert!(matches!(error, ScenesError::Store(StoreError::NotFound(_))));
}

#[tokio::test]
async fn an_unusable_update_leaves_the_prose_as_it_was() {
    let scenes = new_core();
    let scene = scenes
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");
    let id = scene.id().to_string();
    scenes
        .apply(&id, &a_paragraph("The loom stood silent."))
        .await
        .expect("should apply");

    let error = scenes
        .apply(&id, &[255, 255, 255, 255])
        .await
        .expect_err("garbage should be refused");

    assert!(matches!(error, ScenesError::Store(StoreError::Unusable(_))));
    let reopened = scenes.open(&id).await.expect("should still open");
    assert_eq!(reopened.text(), "The loom stood silent.");
}

#[tokio::test]
async fn a_scene_written_by_two_authors_holds_both_contributions() {
    let scenes = new_core();
    let scene = scenes
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");
    let id = scene.id().to_string();

    scenes
        .apply(&id, &a_paragraph("Ada wrote this."))
        .await
        .expect("ada should write");
    scenes
        .apply(&id, &a_paragraph("Bo wrote this."))
        .await
        .expect("bo should write");

    let reopened = scenes.open(&id).await.expect("should open");
    assert!(reopened.text().contains("Ada wrote this."));
    assert!(reopened.text().contains("Bo wrote this."));
}
