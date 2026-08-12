use projects_core::{Project, ProjectId, ProjectName, ProjectStore, StoreError};
use time::{Duration, OffsetDateTime};

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

pub fn sample_project(name: &str, seconds: i64) -> Project {
    Project::new(
        ProjectName::new(name).expect("sample name should be valid"),
        at(seconds),
    )
}

pub async fn create_then_get_returns_the_project(store: &impl ProjectStore) {
    let project = sample_project("Tapestry", 1_000);
    let id = project.id();

    store
        .create(project.clone())
        .await
        .expect("create should succeed");
    let found = store.get(id).await.expect("get should find the project");

    assert_eq!(found, project);
}

pub async fn get_missing_project_is_not_found(store: &impl ProjectStore) {
    let missing = ProjectId::generate(at(1_000));

    let error = store
        .get(missing)
        .await
        .expect_err("get should not find the project");

    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

pub async fn create_rejects_a_duplicate_id(store: &impl ProjectStore) {
    let project = sample_project("Tapestry", 1_000);
    let id = project.id();
    store
        .create(project.clone())
        .await
        .expect("first create should succeed");

    let error = store
        .create(project)
        .await
        .expect_err("second create should conflict");

    assert!(
        matches!(&error, StoreError::Conflict(existing) if *existing == id),
        "expected Conflict({id}), got {error:?}"
    );
}

pub async fn list_is_empty_for_a_fresh_store(store: &impl ProjectStore) {
    let found = store.list().await.expect("list should succeed");

    assert!(
        found.is_empty(),
        "expected no projects in a fresh store, got {found:?}"
    );
}

pub async fn list_returns_projects_in_creation_order(store: &impl ProjectStore) {
    let first = sample_project("First", 1_000);
    let second = sample_project("Second", 2_000);
    let third = sample_project("Third", 3_000);

    for project in [third.clone(), first.clone(), second.clone()] {
        store.create(project).await.expect("create should succeed");
    }

    let found = store.list().await.expect("list should succeed");

    assert_eq!(found, vec![first, second, third]);
}

pub async fn update_replaces_the_stored_project(store: &impl ProjectStore) {
    let mut project = sample_project("Working Title", 1_000);
    let id = project.id();
    store
        .create(project.clone())
        .await
        .expect("create should succeed");

    project.rename(
        ProjectName::new("The Weaver's Apprentice").expect("name should be valid"),
        at(2_000),
    );
    store.update(project).await.expect("update should succeed");

    let found = store.get(id).await.expect("get should find the project");
    assert_eq!(found.name().as_str(), "The Weaver's Apprentice");
    assert_eq!(found.updated_at(), at(2_000));
}

pub async fn update_missing_project_is_not_found(store: &impl ProjectStore) {
    let project = sample_project("Ghost", 1_000);
    let id = project.id();

    let error = store
        .update(project)
        .await
        .expect_err("update should not find the project");

    assert!(
        matches!(&error, StoreError::NotFound(missing) if *missing == id),
        "expected NotFound({id}), got {error:?}"
    );
}

pub async fn delete_removes_the_project(store: &impl ProjectStore) {
    let project = sample_project("Tapestry", 1_000);
    let id = project.id();
    store.create(project).await.expect("create should succeed");

    store.delete(id).await.expect("delete should succeed");

    let found = store.get(id).await;
    assert!(
        matches!(&found, Err(StoreError::NotFound(gone)) if *gone == id),
        "expected NotFound({id}) after delete, got {found:?}"
    );
}

pub async fn delete_missing_project_is_not_found(store: &impl ProjectStore) {
    let missing = ProjectId::generate(at(1_000));

    let error = store
        .delete(missing)
        .await
        .expect_err("delete should not find the project");

    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

macro_rules! conformance_case {
    ($make_store:expr, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            $crate::suite::$case(&$make_store).await;
        }
    };
}

macro_rules! conformance_tests {
    ($make_store:expr) => {
        $crate::suite::conformance_case!($make_store, create_then_get_returns_the_project);
        $crate::suite::conformance_case!($make_store, get_missing_project_is_not_found);
        $crate::suite::conformance_case!($make_store, create_rejects_a_duplicate_id);
        $crate::suite::conformance_case!($make_store, list_is_empty_for_a_fresh_store);
        $crate::suite::conformance_case!($make_store, list_returns_projects_in_creation_order);
        $crate::suite::conformance_case!($make_store, update_replaces_the_stored_project);
        $crate::suite::conformance_case!($make_store, update_missing_project_is_not_found);
        $crate::suite::conformance_case!($make_store, delete_removes_the_project);
        $crate::suite::conformance_case!($make_store, delete_missing_project_is_not_found);
    };
}

pub(crate) use {conformance_case, conformance_tests};
