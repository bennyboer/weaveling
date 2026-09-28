use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{Agent, ServiceError, Version};
use projects_core::{InvalidProjectName, ProjectError, ProjectId, ProjectSummary};
use time::{Duration, OffsetDateTime};

use crate::wiring::{Wired, wired};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn nobody() -> Agent {
    Agent::Anonymous
}

fn a_service() -> Wired {
    a_service_on(Arc::new(FixedClock::new(at(1_000))))
}

fn a_service_on(clock: Arc<FixedClock>) -> Wired {
    wired(clock)
}

async fn a_project(wired: &Wired, named: &str) -> String {
    let id = wired
        .projects
        .start(named, &nobody())
        .await
        .expect("starting should succeed")
        .to_string();
    wired.settle().await;

    id
}

async fn listed(wired: &Wired) -> Vec<ProjectSummary> {
    wired.settle().await;

    wired.projects.list().await.expect("listing should succeed")
}

#[tokio::test]
async fn a_started_project_records_when_it_began() {
    let wired = a_service();

    let id = a_project(&wired, "Tapestry").await;

    let standing = wired.projects.get(&id).await.expect("it should be there");
    assert_eq!(standing.state.created_at(), at(1_000));
    assert_eq!(standing.state.updated_at(), at(1_000));
    assert_eq!(standing.version, Version::of(1));
}

#[tokio::test]
async fn surrounding_whitespace_is_trimmed_from_a_new_name() {
    let wired = a_service();

    let id = a_project(&wired, "  Tapestry  ").await;

    let standing = wired.projects.get(&id).await.expect("it should be there");
    assert_eq!(standing.state.name().as_str(), "Tapestry");
}

#[tokio::test]
async fn starting_a_project_with_a_blank_name_is_refused_and_writes_nothing() {
    let wired = a_service();

    let refused = wired
        .projects
        .start("   ", &nobody())
        .await
        .expect_err("a blank name should be refused");

    assert!(
        matches!(
            &refused,
            projects_core::ProjectServiceError::InvalidName(InvalidProjectName::Blank)
        ),
        "expected a blank-name error, got {refused:?}"
    );
    assert!(
        listed(&wired).await.is_empty(),
        "a name the domain refuses must never reach the log"
    );
}

#[tokio::test]
async fn a_started_project_is_catalogued_without_anyone_asking() {
    let wired = a_service();

    let id = a_project(&wired, "Tapestry").await;

    let found = listed(&wired).await;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id.to_string(), id);
    assert_eq!(found[0].name.as_str(), "Tapestry");
}

#[tokio::test]
async fn renaming_moves_the_moment_the_project_was_last_touched() {
    let clock = Arc::new(FixedClock::new(at(1_000)));
    let wired = a_service_on(clock.clone());
    let id = a_project(&wired, "Working Title").await;

    clock.set(at(2_000));
    wired
        .projects
        .rename(&id, "The Weaver's Apprentice", None, &nobody())
        .await
        .expect("renaming should succeed");

    let standing = wired.projects.get(&id).await.expect("it should be there");
    assert_eq!(standing.state.name().as_str(), "The Weaver's Apprentice");
    assert_eq!(standing.state.created_at(), at(1_000));
    assert_eq!(standing.state.updated_at(), at(2_000));
}

#[tokio::test]
async fn renaming_with_a_malformed_id_is_refused() {
    let wired = a_service();

    let refused = wired
        .projects
        .rename("weaveling", "Renamed", None, &nobody())
        .await
        .expect_err("a malformed id should be refused");

    assert!(
        matches!(&refused, projects_core::ProjectServiceError::InvalidId(_)),
        "expected an invalid-id error, got {refused:?}"
    );
}

#[tokio::test]
async fn renaming_to_a_blank_name_leaves_the_project_as_it_was() {
    let wired = a_service();
    let id = a_project(&wired, "Working Title").await;

    let refused = wired
        .projects
        .rename(&id, "", None, &nobody())
        .await
        .expect_err("a blank name should be refused");

    assert!(
        matches!(
            &refused,
            projects_core::ProjectServiceError::InvalidName(InvalidProjectName::Blank)
        ),
        "expected a blank-name error, got {refused:?}"
    );
    let standing = wired.projects.get(&id).await.expect("it should be there");
    assert_eq!(standing.state.name().as_str(), "Working Title");
    assert_eq!(standing.version, Version::of(1));
}

#[tokio::test]
async fn renaming_a_project_nobody_started_is_not_found() {
    let wired = a_service();
    let missing = ProjectId::generate(at(1_000));

    let refused = wired
        .projects
        .rename(&missing.to_string(), "Renamed", None, &nobody())
        .await
        .expect_err("an unknown project should not be renameable");

    assert!(
        matches!(
            &refused,
            projects_core::ProjectServiceError::Events(ServiceError::NotFound { .. })
        ),
        "expected NotFound, got {refused:?}"
    );
}

#[tokio::test]
async fn a_deleted_project_is_gone_from_the_authors_point_of_view() {
    let wired = a_service();
    let id = a_project(&wired, "Abandoned").await;

    wired
        .projects
        .delete(&id, None, &nobody())
        .await
        .expect("deleting should succeed");

    assert!(
        matches!(
            wired
                .projects
                .get(&id)
                .await
                .expect_err("it should be gone"),
            projects_core::ProjectServiceError::Events(ServiceError::NotFound { .. })
        ),
        "the stream survives as the audit log, but the project itself is no longer served"
    );
    assert!(listed(&wired).await.is_empty());
}

#[tokio::test]
async fn a_deleted_project_cannot_be_renamed_afterwards() {
    let wired = a_service();
    let id = a_project(&wired, "Abandoned").await;
    wired
        .projects
        .delete(&id, None, &nobody())
        .await
        .expect("deleting should succeed");

    let refused = wired
        .projects
        .rename(&id, "Too late", None, &nobody())
        .await
        .expect_err("a deleted project should accept nothing");

    assert!(
        matches!(
            &refused,
            projects_core::ProjectServiceError::Events(ServiceError::Refused(
                ProjectError::Deleted
            ))
        ),
        "expected a refusal that says the project is deleted, got {refused:?}"
    );
}

#[tokio::test]
async fn deleting_a_project_nobody_started_is_not_found() {
    let wired = a_service();
    let missing = ProjectId::generate(at(1_000));

    let refused = wired
        .projects
        .delete(&missing.to_string(), None, &nobody())
        .await
        .expect_err("an unknown project should not be deleteable");

    assert!(
        matches!(
            &refused,
            projects_core::ProjectServiceError::Events(ServiceError::NotFound { .. })
        ),
        "expected NotFound, got {refused:?}"
    );
}

#[tokio::test]
async fn the_project_started_most_recently_is_listed_first() {
    let clock = Arc::new(FixedClock::new(at(1_000)));
    let wired = a_service_on(clock.clone());
    a_project(&wired, "First").await;
    clock.set(at(2_000));
    a_project(&wired, "Second").await;

    let found = listed(&wired).await;

    assert_eq!(
        found
            .iter()
            .map(|summary| summary.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Second", "First"],
        "the project an author touched most recently should be the first they see"
    );
}

#[tokio::test]
async fn renaming_from_a_stale_version_is_refused() {
    let wired = a_service();
    let id = a_project(&wired, "Working Title").await;
    wired
        .projects
        .rename(&id, "Once", None, &nobody())
        .await
        .expect("the first rename should succeed");

    let refused = wired
        .projects
        .rename(&id, "Twice", Some(Version::of(1)), &nobody())
        .await
        .expect_err("the project has moved on since version one");

    assert!(
        matches!(
            &refused,
            projects_core::ProjectServiceError::Events(ServiceError::Store(
                eventsourcing::StoreError::Outdated { .. }
            ))
        ),
        "expected a stale-version refusal, got {refused:?}"
    );
}
