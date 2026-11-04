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
    // given: a service
    let wired = a_service();

    // when: a project is started
    let id = a_project(&wired, "Tapestry").await;

    // then: it was created and updated as it began, at its first version
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
    // given: a service
    let wired = a_service();

    // when: a project is started with a name in margins
    let id = a_project(&wired, "  Tapestry  ").await;

    // then: its name is trimmed
    let standing = wired
        .projects
        .read(&id, &nobody())
        .await
        .expect("it should be there");
    assert_eq!(standing.state.name().as_str(), "Tapestry");
}

#[tokio::test]
async fn starting_a_project_with_a_blank_name_is_refused_and_writes_nothing() {
    // given: a service
    let wired = a_service();

    // when: a project is started with a blank name
    let refused = wired
        .projects
        .start("   ", &RawDetails::default(), &nobody())
        .await
        .expect_err("a blank name should be refused");

    // then: it is refused as blank
    assert!(
        matches!(
            &refused,
            projects_core::ProjectsError::InvalidName(InvalidProjectName::Blank)
        ),
        "expected a blank-name error, got {refused:?}"
    );
    // and: nothing is listed
    assert!(
        listed(&wired).await.is_empty(),
        "a name the domain refuses must never reach the log"
    );
}

#[tokio::test]
async fn a_started_project_is_catalogued_without_anyone_asking() {
    // given: a service
    let wired = a_service();

    // when: a project is started
    let id = a_project(&wired, "Tapestry").await;

    // then: it alone is listed, by its name
    let found = listed(&wired).await;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id.to_string(), id);
    assert_eq!(found[0].name.as_str(), "Tapestry");
}

#[tokio::test]
async fn renaming_moves_the_moment_the_project_was_last_touched() {
    // given: a started project
    let clock = Arc::new(FixedClock::new(at(1_000)));
    let wired = a_service_on(clock.clone());
    let id = a_project(&wired, "Working Title").await;

    // when: it is renamed later
    clock.set(at(2_000));
    wired
        .projects
        .rename(&id, "The Weaver's Apprentice", None, &nobody())
        .await
        .expect("renaming should succeed");

    // then: it has the new name, updated as of then and created as before
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
    // given: a service
    let wired = a_service();

    // when: something that is no id is renamed
    let refused = wired
        .projects
        .rename("weaveling", "Renamed", None, &nobody())
        .await
        .expect_err("a malformed id should be refused");

    // then: it is refused as an invalid id
    assert!(
        matches!(&refused, projects_core::ProjectsError::InvalidId(_)),
        "expected an invalid-id error, got {refused:?}"
    );
}

#[tokio::test]
async fn renaming_to_a_blank_name_leaves_the_project_as_it_was() {
    // given: a started project
    let wired = a_service();
    let id = a_project(&wired, "Working Title").await;

    // when: it is renamed to a blank name
    let refused = wired
        .projects
        .rename(&id, "", None, &nobody())
        .await
        .expect_err("a blank name should be refused");

    // then: it is refused as blank
    assert!(
        matches!(
            &refused,
            projects_core::ProjectsError::InvalidName(InvalidProjectName::Blank)
        ),
        "expected a blank-name error, got {refused:?}"
    );
    // and: the project keeps its name and version
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
    // given: a project nobody started
    let wired = a_service();
    let missing = ProjectId::generate(at(1_000));

    // when: it is renamed
    let refused = wired
        .projects
        .rename(&missing.to_string(), "Renamed", None, &nobody())
        .await
        .expect_err("an unknown project should not be renameable");

    // then: it is not found
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
    // given: a started project
    let wired = a_service();
    let id = a_project(&wired, "Abandoned").await;

    // when: it is trashed
    wired
        .projects
        .trash(&id, None, &nobody())
        .await
        .expect("trashing should succeed");

    // then: it is no longer found
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
    // and: it is no longer listed
    assert!(listed(&wired).await.is_empty());
}

#[tokio::test]
async fn a_trashed_project_cannot_be_renamed() {
    // given: a trashed project
    let wired = a_service();
    let id = a_project(&wired, "Abandoned").await;
    wired
        .projects
        .trash(&id, None, &nobody())
        .await
        .expect("trashing should succeed");

    // when: it is renamed
    let refused = wired
        .projects
        .rename(&id, "Too late", None, &nobody())
        .await
        .expect_err("a trashed project should accept nothing");

    // then: it is refused as in the trash
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
    // given: a trashed and purged project
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

    // when: it is renamed
    let refused = wired
        .projects
        .rename(&id, "Too late", None, &nobody())
        .await
        .expect_err("a deleted project should accept nothing");

    // then: it is refused as deleted
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
    // given: a project nobody started
    let wired = a_service();
    let missing = ProjectId::generate(at(1_000));

    // when: it is trashed
    let refused = wired
        .projects
        .trash(&missing.to_string(), None, &nobody())
        .await
        .expect_err("an unknown project should not be trashable");

    // then: it is not found
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
    // given: a project started
    let clock = Arc::new(FixedClock::new(at(1_000)));
    let wired = a_service_on(clock.clone());
    a_project(&wired, "First").await;
    // and: another started later
    clock.set(at(2_000));
    a_project(&wired, "Second").await;

    // when: the projects are listed
    let found = listed(&wired).await;

    // then: the later one comes first
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
    // given: a project renamed once
    let wired = a_service();
    let id = a_project(&wired, "Working Title").await;
    wired
        .projects
        .rename(&id, "Once", None, &nobody())
        .await
        .expect("the first rename should succeed");

    // when: it is renamed from the version before
    let refused = wired
        .projects
        .rename(&id, "Twice", Some(Version::of(1)), &nobody())
        .await
        .expect_err("the project has moved on since version one");

    // then: it is refused as outdated
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

async fn renaming_a_project_while(
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
    // when: a project is renamed while it loses four races
    let (renamed, name, _) =
        renaming_a_project_while(|store| FailsAppending::losing_races(store, 4), None).await;

    // then: the renaming lands
    assert_eq!(renamed.expect("the fifth try lands"), Version::of(2));
    assert_eq!(name, "Loom");
}

#[tokio::test]
async fn a_rename_that_lost_every_race_it_may_try_is_a_conflict() {
    // when: a project is renamed while it loses more races than it may try
    let (renamed, name, left) =
        renaming_a_project_while(|store| FailsAppending::losing_races(store, 6), None).await;

    // then: it is refused as outdated
    assert!(matches!(
        renamed,
        Err(ProjectsError::Events(ServiceError::Store(
            StoreError::Outdated { .. }
        )))
    ));
    // and: the project keeps its name after five tries
    assert_eq!(
        (name.as_str(), left),
        ("Tapestry", 1),
        "five tries, no more"
    );
}

#[tokio::test]
async fn a_rename_aimed_at_a_version_that_lost_a_race_is_not_tried_again() {
    // when: a project is renamed from a version while it loses a race
    let (renamed, name, left) = renaming_a_project_while(
        |store| FailsAppending::losing_races(store, 2),
        Some(Version::of(1)),
    )
    .await;

    // then: it is refused as outdated
    assert!(matches!(
        renamed,
        Err(ProjectsError::Events(ServiceError::Store(
            StoreError::Outdated { .. }
        )))
    ));
    // and: it was tried once and keeps its name
    assert_eq!(
        (name.as_str(), left),
        ("Tapestry", 1),
        "whoever named a version hears that it moved on, a 412, rather than having it papered over"
    );
}

#[tokio::test]
async fn a_rename_whose_store_broke_is_not_tried_again() {
    // when: a project is renamed while its store is broken
    let (renamed, _, left) =
        renaming_a_project_while(|store| FailsAppending::broken(store, 2), None).await;

    // then: it fails as the store did
    assert!(matches!(
        renamed,
        Err(ProjectsError::Events(ServiceError::Store(
            StoreError::Backend { .. }
        )))
    ));
    // and: it was tried once
    assert_eq!(left, 1, "only a lost race is tried again");
}

#[tokio::test]
async fn a_rename_aimed_at_the_version_it_stands_at_lands() {
    // when: a project is renamed from the version it stands at
    let (renamed, name, _) = renaming_a_project_while(
        |store| FailsAppending::losing_races(store, 0),
        Some(Version::of(1)),
    )
    .await;

    // then: the renaming lands
    assert_eq!(
        (renamed.expect("renaming should succeed"), name.as_str()),
        (Version::of(2), "Loom")
    );
}
