use std::sync::Arc;

use authorizing::AllowingEverything;
use clock::FixedClock;
use eventsourcing::{Agent, FailsAppending, InMemoryEventStore, ServiceError, StoreError, Version};
use projects_catalog::InMemoryProjectCatalog;
use projects_core::{
    InvalidProjectName, ProjectError, ProjectEvent, ProjectId, ProjectSummary, ProjectsCore,
    ProjectsError, RawDetails,
};
use time::{Duration, OffsetDateTime};
use transacting::InMemoryTransactions;
use trash::InMemoryTrash;

use crate::wiring;
use crate::wiring::Wired;

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn nobody() -> Agent {
    Agent::Local
}

fn a_service() -> Wired {
    a_service_on(Arc::new(FixedClock::new(at(1_000))))
}

fn a_service_on(clock: Arc<FixedClock>) -> Wired {
    wiring::wired(clock)
}

async fn a_project(wired: &Wired, named: &str) -> String {
    let id = wired
        .projects
        .start(named, &RawDetails::default(), &nobody())
        .await
        .expect("starting should succeed")
        .0
        .to_string();
    wired.settle().await;

    id
}

async fn listed(wired: &Wired) -> Vec<ProjectSummary> {
    wired.settle().await;

    wired
        .projects
        .list(&nobody())
        .await
        .expect("listing should succeed")
}

#[tokio::test]
async fn a_started_project_records_when_it_began() {
    let wired = a_service();

    let id = a_project(&wired, "Tapestry").await;

    let standing = wired
        .projects
        .read(&id, &nobody())
        .await
        .expect("it should be there");
    assert_eq!(standing.state.created_at(), at(1_000));
    assert_eq!(standing.state.updated_at(), at(1_000));
    assert_eq!(standing.version, Version::of(1));
}

#[tokio::test]
async fn surrounding_whitespace_is_trimmed_from_a_new_name() {
    let wired = a_service();

    let id = a_project(&wired, "  Tapestry  ").await;

    let standing = wired
        .projects
        .read(&id, &nobody())
        .await
        .expect("it should be there");
    assert_eq!(standing.state.name().as_str(), "Tapestry");
}

#[tokio::test]
async fn starting_a_project_with_a_blank_name_is_refused_and_writes_nothing() {
    let wired = a_service();

    let refused = wired
        .projects
        .start("   ", &RawDetails::default(), &nobody())
        .await
        .expect_err("a blank name should be refused");

    assert!(
        matches!(
            &refused,
            projects_core::ProjectsError::InvalidName(InvalidProjectName::Blank)
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

    let standing = wired
        .projects
        .read(&id, &nobody())
        .await
        .expect("it should be there");
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
        matches!(&refused, projects_core::ProjectsError::InvalidId(_)),
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
            projects_core::ProjectsError::InvalidName(InvalidProjectName::Blank)
        ),
        "expected a blank-name error, got {refused:?}"
    );
    let standing = wired
        .projects
        .read(&id, &nobody())
        .await
        .expect("it should be there");
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
            projects_core::ProjectsError::Events(ServiceError::NotFound { .. })
        ),
        "expected NotFound, got {refused:?}"
    );
}

#[tokio::test]
async fn a_trashed_project_is_gone_from_the_authors_point_of_view() {
    let wired = a_service();
    let id = a_project(&wired, "Abandoned").await;

    wired
        .projects
        .trash(&id, None, &nobody())
        .await
        .expect("trashing should succeed");

    assert!(
        matches!(
            wired
                .projects
                .read(&id, &nobody())
                .await
                .expect_err("it should be gone"),
            projects_core::ProjectsError::Events(ServiceError::NotFound { .. })
        ),
        "in the trash, the project is no longer served"
    );
    assert!(listed(&wired).await.is_empty());
}

#[tokio::test]
async fn a_trashed_project_cannot_be_renamed() {
    let wired = a_service();
    let id = a_project(&wired, "Abandoned").await;
    wired
        .projects
        .trash(&id, None, &nobody())
        .await
        .expect("trashing should succeed");

    let refused = wired
        .projects
        .rename(&id, "Too late", None, &nobody())
        .await
        .expect_err("a trashed project should accept nothing");

    assert!(
        matches!(
            &refused,
            projects_core::ProjectsError::Events(ServiceError::Refused(ProjectError::Trashed))
        ),
        "expected a refusal that says the project is in the trash, got {refused:?}"
    );
}

#[tokio::test]
async fn a_purged_project_cannot_be_renamed_afterwards() {
    let wired = a_service();
    let id = a_project(&wired, "Abandoned").await;
    wired
        .projects
        .trash(&id, None, &nobody())
        .await
        .expect("trashing should succeed");
    wired
        .projects
        .purge(&id)
        .await
        .expect("purging should succeed");

    let refused = wired
        .projects
        .rename(&id, "Too late", None, &nobody())
        .await
        .expect_err("a deleted project should accept nothing");

    assert!(
        matches!(
            &refused,
            projects_core::ProjectsError::Events(ServiceError::Refused(ProjectError::Deleted))
        ),
        "expected a refusal that says the project is deleted, got {refused:?}"
    );
}

#[tokio::test]
async fn trashing_a_project_nobody_started_is_not_found() {
    let wired = a_service();
    let missing = ProjectId::generate(at(1_000));

    let refused = wired
        .projects
        .trash(&missing.to_string(), None, &nobody())
        .await
        .expect_err("an unknown project should not be trashable");

    assert!(
        matches!(
            &refused,
            projects_core::ProjectsError::Events(ServiceError::NotFound { .. })
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
            projects_core::ProjectsError::Events(ServiceError::Store(
                eventsourcing::StoreError::Outdated { .. }
            ))
        ),
        "expected a stale-version refusal, got {refused:?}"
    );
}

async fn a_project_renamed_while_failing(
    failing: fn(Arc<InMemoryEventStore<ProjectEvent>>) -> FailsAppending<ProjectEvent>,
    expected: Option<Version>,
) -> (Result<Version, ProjectsError>, String, u8) {
    let store = Arc::new(InMemoryEventStore::new());
    let core_over = |store| {
        ProjectsCore::new(
            store,
            Arc::new(InMemoryTransactions::new()),
            Arc::new(InMemoryProjectCatalog::new()),
            Arc::new(InMemoryTrash::new()),
            Arc::new(AllowingEverything),
            Arc::new(FixedClock::new(at(1_000))),
        )
    };
    let (id, _) = core_over(store.clone())
        .start("Tapestry", &RawDetails::default(), &nobody())
        .await
        .expect("starting should succeed");
    let losing = Arc::new(failing(store.clone()));

    let renamed = core_over(losing.clone())
        .rename(&id.to_string(), "Loom", expected, &nobody())
        .await
        .map(|standing| standing.version);
    let name = core_over(store)
        .read(&id.to_string(), &nobody())
        .await
        .expect("it should be there")
        .state
        .name()
        .as_str()
        .to_owned();

    (renamed, name, losing.left())
}

#[tokio::test]
async fn a_rename_that_lost_a_race_is_tried_again_and_lands() {
    let (renamed, name, _) =
        a_project_renamed_while_failing(|store| FailsAppending::losing_races(store, 4), None).await;

    assert_eq!(renamed.expect("the fifth try lands"), Version::of(2));
    assert_eq!(name, "Loom");
}

#[tokio::test]
async fn a_rename_that_lost_every_race_it_may_try_is_a_conflict() {
    let (renamed, name, left) =
        a_project_renamed_while_failing(|store| FailsAppending::losing_races(store, 6), None).await;

    assert!(matches!(
        renamed,
        Err(ProjectsError::Events(ServiceError::Store(
            StoreError::Outdated { .. }
        )))
    ));
    assert_eq!(
        (name.as_str(), left),
        ("Tapestry", 1),
        "five tries, no more"
    );
}

#[tokio::test]
async fn a_rename_aimed_at_a_version_that_lost_a_race_is_not_tried_again() {
    let (renamed, name, left) = a_project_renamed_while_failing(
        |store| FailsAppending::losing_races(store, 2),
        Some(Version::of(1)),
    )
    .await;

    assert!(matches!(
        renamed,
        Err(ProjectsError::Events(ServiceError::Store(
            StoreError::Outdated { .. }
        )))
    ));
    assert_eq!(
        (name.as_str(), left),
        ("Tapestry", 1),
        "whoever named a version hears that it moved on, a 412, rather than having it papered over"
    );
}

#[tokio::test]
async fn a_rename_whose_store_broke_is_not_tried_again() {
    let (renamed, _, left) =
        a_project_renamed_while_failing(|store| FailsAppending::broken(store, 2), None).await;

    assert!(matches!(
        renamed,
        Err(ProjectsError::Events(ServiceError::Store(
            StoreError::Backend { .. }
        )))
    ));
    assert_eq!(left, 1, "only a lost race is tried again");
}

#[tokio::test]
async fn a_rename_aimed_at_the_version_it_stands_at_lands() {
    let (renamed, name, _) = a_project_renamed_while_failing(
        |store| FailsAppending::losing_races(store, 0),
        Some(Version::of(1)),
    )
    .await;

    assert_eq!(
        (renamed.expect("renaming should succeed"), name.as_str()),
        (Version::of(2), "Loom")
    );
}
