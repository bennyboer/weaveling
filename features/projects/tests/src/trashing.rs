use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use authorizing::{Act, AllowingEverything, Authorizer, AuthorizerError, Readable};
use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, InMemoryEventStore, ServiceError};
use projects_catalog::InMemoryProjectCatalog;
use projects_contract::{TrashedProjectDTO, refused};
use projects_core::{
    KEPT_IN_THE_TRASH_FOR, ProjectError, ProjectSummary, ProjectsCore, ProjectsError, RawDetails,
};
use serde_json::Value;
use time::{Duration, OffsetDateTime};
use transacting::InMemoryTransactions;
use trash::{InMemoryTrash, Trash, Trashed};

use crate::wiring;
use crate::wiring::Wired;

const ADA: &str = "account_ada";
const GRACE: &str = "account_grace";

fn local() -> Agent {
    Agent::Local
}

fn an_account(id: &str) -> Agent {
    Agent::Account(AccountId::from(id))
}

#[derive(Default)]
struct Roles {
    held: Mutex<HashMap<(String, String), Vec<Act>>>,
}

impl Roles {
    fn give(&self, account: &str, project: &str, acts: &[Act]) {
        self.held
            .lock()
            .expect("never poisoned")
            .insert((account.to_owned(), project.to_owned()), acts.to_vec());
    }
}

#[async_trait]
impl Authorizer for Roles {
    async fn may(&self, agent: &Agent, act: Act, project: &str) -> Result<bool, AuthorizerError> {
        let Agent::Account(account) = agent else {
            return Ok(false);
        };

        Ok(self
            .held
            .lock()
            .expect("never poisoned")
            .get(&(account.to_string(), project.to_owned()))
            .is_some_and(|acts| acts.contains(&act)))
    }

    async fn readable_by(&self, _agent: &Agent) -> Result<Readable, AuthorizerError> {
        Ok(Readable::Everything)
    }
}

const OWNER: [Act; 4] = [Act::Read, Act::Write, Act::Manage, Act::Restore];
const AUTHOR: [Act; 2] = [Act::Read, Act::Write];
const IN_THE_TRASH_AN_OWNER_MAY: [Act; 1] = [Act::Restore];

struct Bench {
    wired: Wired,
    clock: Arc<FixedClock>,
}

fn bench() -> Bench {
    let clock = Arc::new(FixedClock::new(clock::at(1_000)));

    Bench {
        wired: wiring::wired(clock.clone()),
        clock,
    }
}

impl Bench {
    async fn a_project(&self, named: &str) -> String {
        let id = self
            .wired
            .projects
            .start(named, &RawDetails::default(), &local())
            .await
            .expect("starting should succeed")
            .0
            .to_string();
        self.wired.settle().await;

        id
    }

    async fn trashed(&self, id: &str) {
        self.wired
            .projects
            .trash(id, None, &local())
            .await
            .expect("trashing should succeed");
        self.wired.settle().await;
    }

    async fn due(&self, by: OffsetDateTime) -> Vec<Trashed> {
        self.wired
            .trash
            .due(by, 100)
            .await
            .expect("asking the trash should succeed")
    }

    async fn in_the_trash(&self) -> Vec<ProjectSummary> {
        self.wired
            .projects
            .trash_list(&local())
            .await
            .expect("listing the trash should succeed")
    }

    async fn listed(&self) -> Vec<ProjectSummary> {
        self.wired
            .projects
            .list(&local())
            .await
            .expect("listing should succeed")
    }
}

fn a_project_trashed(id: &str) -> Trashed {
    Trashed {
        kind: "project".to_owned(),
        id: id.to_owned(),
    }
}

fn the_period() -> OffsetDateTime {
    clock::at(1_000) + KEPT_IN_THE_TRASH_FOR
}

#[tokio::test]
async fn a_project_deleted_goes_to_the_trash_until_its_time_there_is_over() {
    // given: a project
    let bench = bench();
    let id = bench.a_project("Tapestry").await;

    // when: it is trashed
    bench.trashed(&id).await;

    // then: it is kept in the trash for thirty days
    assert_eq!(KEPT_IN_THE_TRASH_FOR, Duration::days(30));
    // and: it is not due a moment before they are over
    assert!(
        bench
            .due(the_period() - Duration::seconds(1))
            .await
            .is_empty(),
        "not due before its thirty days are over"
    );
    // and: it is due once they are
    assert_eq!(bench.due(the_period()).await, [a_project_trashed(&id)]);
    // and: the trash lists it until then
    let in_the_trash = bench.in_the_trash().await;
    assert_eq!(in_the_trash.len(), 1);
    assert_eq!(in_the_trash[0].trashed_until, Some(the_period()));
    // and: it is not listed
    assert!(
        bench.listed().await.is_empty(),
        "a project in the trash is not listed"
    );
}

#[tokio::test]
async fn a_project_in_the_trash_is_read_by_nobody() {
    // given: a project
    let bench = bench();
    let id = bench.a_project("Tapestry").await;

    // when: it is trashed
    bench.trashed(&id).await;

    // then: it is not found
    assert!(matches!(
        bench.wired.projects.read(&id, &local()).await,
        Err(ProjectsError::Events(ServiceError::NotFound { .. }))
    ));
}

#[tokio::test]
async fn trashing_a_project_twice_keeps_it_until_the_first_time_said() {
    // given: a project trashed
    let bench = bench();
    let id = bench.a_project("Tapestry").await;
    bench.trashed(&id).await;
    // and: ten days gone by
    bench.clock.set(clock::at(1_000) + Duration::days(10));

    // when: it is trashed again
    bench.trashed(&id).await;

    // then: it is due when the first trashing said
    assert_eq!(
        bench.due(the_period()).await,
        [a_project_trashed(&id)],
        "a second trashing is no failure and buys no time"
    );
}

#[tokio::test]
async fn a_trashing_that_does_not_commit_leaves_neither_the_event_nor_the_entry() {
    // given: a started project
    let store = Arc::new(InMemoryEventStore::new());
    let trash = Arc::new(InMemoryTrash::new());
    let core_on = |transactions: InMemoryTransactions| {
        ProjectsCore::new(
            store.clone(),
            Arc::new(transactions),
            Arc::new(InMemoryProjectCatalog::new()),
            trash.clone(),
            Arc::new(AllowingEverything),
            Arc::new(FixedClock::new(clock::at(1_000))),
        )
    };
    let (id, _) = core_on(InMemoryTransactions::new())
        .start("Tapestry", &RawDetails::default(), &local())
        .await
        .expect("starting should succeed");

    // when: it is trashed in a transaction that does not commit
    core_on(InMemoryTransactions::refusing_commits())
        .trash(&id.to_string(), None, &local())
        .await
        .expect_err("a transaction that does not commit fails the trashing");

    // then: nothing is in the trash
    assert!(
        trash
            .due(the_period(), 100)
            .await
            .expect("asking")
            .is_empty()
    );
    // and: the project is still found
    assert!(
        core_on(InMemoryTransactions::new())
            .read(&id.to_string(), &local())
            .await
            .is_ok(),
        "the project was never trashed"
    );
}

#[tokio::test]
async fn a_project_restored_is_back_and_out_of_the_trash() {
    // given: a trashed project
    let bench = bench();
    let id = bench.a_project("Tapestry").await;
    bench.trashed(&id).await;

    // when: it is restored
    bench
        .wired
        .projects
        .restore(&id, &local())
        .await
        .expect("restoring should succeed");
    bench.wired.settle().await;

    // then: it is neither due nor in the trash
    assert!(bench.due(the_period()).await.is_empty());
    assert!(bench.in_the_trash().await.is_empty());
    // and: it is listed
    assert_eq!(bench.listed().await.len(), 1);
    // and: it takes changes again
    bench
        .wired
        .projects
        .rename(&id, "Rewoven", None, &local())
        .await
        .expect("a project restored takes changes again");
}

#[tokio::test]
async fn a_project_not_in_the_trash_cannot_be_restored() {
    // given: a project out of the trash
    let bench = bench();
    let id = bench.a_project("Tapestry").await;

    // expect: restoring it is refused as not in the trash
    assert!(matches!(
        bench.wired.projects.restore(&id, &local()).await,
        Err(ProjectsError::Events(ServiceError::Refused(
            ProjectError::NotTrashed
        )))
    ));
}

#[tokio::test]
async fn a_project_whose_time_in_the_trash_is_over_is_purged_and_deleted() {
    // given: a trashed project
    let bench = bench();
    let id = bench.a_project("Tapestry").await;
    bench.trashed(&id).await;

    // when: a moment of its time in the trash is left
    bench.clock.set(the_period() - Duration::seconds(1));
    // then: a purge takes nothing
    assert_eq!(
        bench.wired.purged().await,
        0,
        "nothing purged before its time"
    );

    // when: its time is over and the trash is purged
    bench.clock.set(the_period());
    let purged = bench.wired.purged().await;

    // then: it is purged
    assert_eq!(purged, 1);
    // and: it is no longer due
    assert!(
        bench.due(the_period()).await.is_empty(),
        "taken out with the purge"
    );
    // and: the trash no longer lists it
    assert!(
        bench.in_the_trash().await.is_empty(),
        "the deletion was told and the list forgot the project"
    );
    // and: it is deleted
    assert!(matches!(
        bench
            .wired
            .projects
            .rename(&id, "Too late", None, &local())
            .await,
        Err(ProjectsError::Events(ServiceError::Refused(
            ProjectError::Deleted
        )))
    ));
}

#[tokio::test]
async fn a_project_restored_before_its_time_is_over_is_not_purged() {
    // given: a project trashed and restored
    let bench = bench();
    let id = bench.a_project("Tapestry").await;
    bench.trashed(&id).await;
    bench
        .wired
        .projects
        .restore(&id, &local())
        .await
        .expect("restoring should succeed");
    // and: its time in the trash long over
    bench.clock.set(the_period() + Duration::days(1));

    // expect: nothing is purged
    assert_eq!(bench.wired.purged().await, 0);
    // and: it is still found
    assert!(bench.wired.projects.read(&id, &local()).await.is_ok());
}

#[tokio::test]
async fn a_purge_of_a_project_restored_meanwhile_only_takes_its_entry_out() {
    // given: a project trashed and restored
    let bench = bench();
    let id = bench.a_project("Tapestry").await;
    bench.trashed(&id).await;
    bench
        .wired
        .projects
        .restore(&id, &local())
        .await
        .expect("restoring should succeed");

    // when: it is purged
    bench
        .wired
        .projects
        .purge(&id)
        .await
        .expect("a purge that finds the project restored is no failure");

    // then: it is still found
    assert!(bench.wired.projects.read(&id, &local()).await.is_ok());
}

#[tokio::test]
async fn only_an_owner_trashes_restores_and_sees_the_trash() {
    // given: a project ada owns and grace authors
    let roles = Arc::new(Roles::default());
    let wired =
        wiring::wired_guarded_by(Arc::new(FixedClock::new(clock::at(1_000))), roles.clone());
    let id = wired
        .projects
        .start("Shared", &RawDetails::default(), &an_account(ADA))
        .await
        .expect("starting should succeed")
        .0
        .to_string();
    roles.give(ADA, &id, &OWNER);
    roles.give(GRACE, &id, &AUTHOR);

    // expect: grace may not trash it
    assert!(matches!(
        wired.projects.trash(&id, None, &an_account(GRACE)).await,
        Err(ProjectsError::NotAnOwner)
    ));

    // when: ada trashes it
    wired
        .projects
        .trash(&id, None, &an_account(ADA))
        .await
        .expect("an owner trashes");
    wired.settle().await;
    roles.give(ADA, &id, &IN_THE_TRASH_AN_OWNER_MAY);

    // then: ada sees it in the trash and grace does not
    let seen_by = |account: &'static str| {
        let projects = wired.projects.clone();
        async move {
            projects
                .trash_list(&an_account(account))
                .await
                .expect("listing the trash should succeed")
                .len()
        }
    };
    assert_eq!(seen_by(ADA).await, 1);
    assert_eq!(seen_by(GRACE).await, 0, "an author does not see the trash");
    // and: grace may not restore it
    assert!(matches!(
        wired.projects.restore(&id, &an_account(GRACE)).await,
        Err(ProjectsError::NotAnOwner)
    ));
    // and: ada restores it
    wired
        .projects
        .restore(&id, &an_account(ADA))
        .await
        .expect("an owner restores");
}

#[tokio::test]
async fn an_owner_changing_a_project_in_the_trash_hears_it_is_there_and_others_that_it_is_gone() {
    // given: a project ada owns, trashed
    let roles = Arc::new(Roles::default());
    let wired =
        wiring::wired_guarded_by(Arc::new(FixedClock::new(clock::at(1_000))), roles.clone());
    let id = wired
        .projects
        .start("Shared", &RawDetails::default(), &an_account(ADA))
        .await
        .expect("starting should succeed")
        .0
        .to_string();
    roles.give(ADA, &id, &OWNER);
    wired
        .projects
        .trash(&id, None, &an_account(ADA))
        .await
        .expect("an owner trashes");
    wired.settle().await;
    roles.give(ADA, &id, &IN_THE_TRASH_AN_OWNER_MAY);
    // and: grace with no say in it
    roles.give(GRACE, &id, &[]);

    // expect: ada's renaming is refused as in the trash
    assert!(
        matches!(
            wired
                .projects
                .rename(&id, "Renamed", None, &an_account(ADA))
                .await,
            Err(ProjectsError::Events(ServiceError::Refused(
                ProjectError::Trashed
            )))
        ),
        "an owner may restore the project, so a change is refused as in the trash, not as unknown"
    );
    // and: ada's reading finds nothing
    assert!(
        matches!(
            wired.projects.read(&id, &an_account(ADA)).await,
            Err(ProjectsError::Events(ServiceError::NotFound { .. }))
        ),
        "a project in the trash is read by nobody, its owners too"
    );
    // and: grace's renaming finds nothing
    assert!(
        matches!(
            wired
                .projects
                .rename(&id, "Renamed", None, &an_account(GRACE))
                .await,
            Err(ProjectsError::Events(ServiceError::NotFound { .. }))
        ),
        "whoever may not restore it learns nothing of the trash"
    );
}

fn server_of(wired: &Wired) -> TestServer {
    TestServer::new(wired.routes.clone().layer(Extension(local())))
}

#[tokio::test]
async fn deleting_over_rest_trashes_and_restoring_brings_it_back() {
    // given: a project served
    let bench = bench();
    let id = bench.a_project("Tapestry").await;
    let server = server_of(&bench.wired);

    // when: it is deleted
    server
        .delete(&format!("/projects/{id}"))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    bench.wired.settle().await;
    // then: it is not found
    server
        .get(&format!("/projects/{id}"))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    // and: the trash lists it with its name and how long it stays
    let in_the_trash: Vec<TrashedProjectDTO> = server.get("/projects/trash").await.json();
    assert_eq!(in_the_trash.len(), 1);
    assert_eq!(in_the_trash[0].id, id);
    assert_eq!(in_the_trash[0].name, "Tapestry");
    assert_eq!(in_the_trash[0].until, clock::text::serialize(the_period()));

    // when: it is restored
    server
        .post(&format!("/projects/{id}/restore"))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    // and: restored again
    let again = server.post(&format!("/projects/{id}/restore")).await;
    // then: the second restoring is refused as not in the trash
    again.assert_status(StatusCode::CONFLICT);
    assert_eq!(again.json::<Value>()["type"], refused::NOT_TRASHED);
    // and: it is found
    server
        .get(&format!("/projects/{id}"))
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn restoring_a_project_nobody_started_is_not_found() {
    // given: a server
    let bench = bench();
    let server = server_of(&bench.wired);

    // expect: restoring a project nobody started is not found
    server
        .post("/projects/project_0000000000000000000000/restore")
        .await
        .assert_status(StatusCode::NOT_FOUND);
}
