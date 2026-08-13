use std::sync::Arc;

use clock::FixedClock;
use projects_core::{InvalidProjectName, ProjectError, ProjectId, ProjectService, StoreError};
use projects_store::InMemoryProjectStore;
use time::{Duration, OffsetDateTime};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn new_service() -> ProjectService {
    new_service_with(Arc::new(FixedClock::new(at(1_000))))
}

fn new_service_with(clock: Arc<FixedClock>) -> ProjectService {
    ProjectService::new(Arc::new(InMemoryProjectStore::new()), clock)
}

#[tokio::test]
async fn a_new_project_records_when_it_was_created() {
    let service = new_service();

    let project = service.create("Tapestry").await.expect("should create");

    assert_eq!(project.created_at(), at(1_000));
    assert_eq!(project.updated_at(), at(1_000));
}

#[tokio::test]
async fn surrounding_whitespace_is_trimmed_from_a_new_name() {
    let service = new_service();

    let project = service.create("  Tapestry  ").await.expect("should create");

    assert_eq!(project.name().as_str(), "Tapestry");
}

#[tokio::test]
async fn creating_a_project_with_a_blank_name_is_rejected_and_stores_nothing() {
    let service = new_service();

    let error = service.create("   ").await.expect_err("should reject");

    assert!(
        matches!(&error, ProjectError::InvalidName(InvalidProjectName::Blank)),
        "expected a blank-name error, got {error:?}"
    );
    let found = service.list().await.expect("should list");
    assert!(found.is_empty(), "expected no projects, got {found:?}");
}

#[tokio::test]
async fn a_created_project_can_be_found_again() {
    let service = new_service();

    let created = service.create("Tapestry").await.expect("should create");

    let found = service
        .get(&created.id().to_string())
        .await
        .expect("should find it");
    assert_eq!(found, created);
}

#[tokio::test]
async fn renaming_records_when_it_happened() {
    let clock = Arc::new(FixedClock::new(at(1_000)));
    let service = new_service_with(clock.clone());
    let created = service
        .create("Working Title")
        .await
        .expect("should create");

    clock.set(at(2_000));
    let renamed = service
        .rename(&created.id().to_string(), "The Weaver's Apprentice")
        .await
        .expect("should rename");

    assert_eq!(renamed.name().as_str(), "The Weaver's Apprentice");
    assert_eq!(renamed.created_at(), at(1_000));
    assert_eq!(renamed.updated_at(), at(2_000));
}

#[tokio::test]
async fn renaming_with_a_malformed_id_is_rejected() {
    let service = new_service();

    let error = service
        .rename("weaveling", "Renamed")
        .await
        .expect_err("should reject");

    assert!(
        matches!(&error, ProjectError::InvalidId(_)),
        "expected an invalid-id error, got {error:?}"
    );
}

#[tokio::test]
async fn renaming_to_a_blank_name_leaves_the_project_unchanged() {
    let clock = Arc::new(FixedClock::new(at(1_000)));
    let service = new_service_with(clock.clone());
    let created = service
        .create("Working Title")
        .await
        .expect("should create");

    clock.set(at(2_000));
    let error = service
        .rename(&created.id().to_string(), "")
        .await
        .expect_err("should reject");

    assert!(
        matches!(&error, ProjectError::InvalidName(InvalidProjectName::Blank)),
        "expected a blank-name error, got {error:?}"
    );
    let unchanged = service
        .get(&created.id().to_string())
        .await
        .expect("should still exist");
    assert_eq!(unchanged, created);
}

#[tokio::test]
async fn renaming_a_missing_project_reports_not_found() {
    let service = new_service();
    let missing = ProjectId::generate(at(1_000));

    let error = service
        .rename(&missing.to_string(), "Renamed")
        .await
        .expect_err("should fail");

    assert!(
        matches!(&error, ProjectError::Store(StoreError::NotFound(id)) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

#[tokio::test]
async fn deleting_with_a_malformed_id_is_rejected() {
    let service = new_service();

    let error = service
        .delete("weaveling")
        .await
        .expect_err("should reject");

    assert!(
        matches!(&error, ProjectError::InvalidId(_)),
        "expected an invalid-id error, got {error:?}"
    );
}

#[tokio::test]
async fn a_deleted_project_can_no_longer_be_found() {
    let service = new_service();
    let created = service.create("Tapestry").await.expect("should create");

    service
        .delete(&created.id().to_string())
        .await
        .expect("should delete");

    let error = service
        .get(&created.id().to_string())
        .await
        .expect_err("should be gone");

    assert!(
        matches!(&error, ProjectError::Store(StoreError::NotFound(_))),
        "expected NotFound, got {error:?}"
    );
}

#[tokio::test]
async fn deleting_a_missing_project_reports_not_found() {
    let service = new_service();
    let missing = ProjectId::generate(at(1_000));

    let error = service
        .delete(&missing.to_string())
        .await
        .expect_err("should fail");

    assert!(
        matches!(&error, ProjectError::Store(StoreError::NotFound(id)) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

#[tokio::test]
async fn projects_are_listed_in_creation_order() {
    let clock = Arc::new(FixedClock::new(at(1_000)));
    let service = new_service_with(clock.clone());
    let first = service.create("First").await.expect("should create");
    clock.set(at(2_000));
    let second = service.create("Second").await.expect("should create");

    let found = service.list().await.expect("should list");

    assert_eq!(found, vec![first, second]);
}
