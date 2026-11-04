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

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

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
    let clock = Arc::new(FixedClock::new(at(1_000)));

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
    at(1_000) + KEPT_IN_THE_TRASH_FOR
}

#[tokio::test]
async fn a_project_deleted_goes_to_the_trash_until_its_time_there_is_over() {
    let bench = bench();
    let id = bench.a_project("Tapestry").await;

    bench.trashed(&id).await;

    assert_eq!(KEPT_IN_THE_TRASH_FOR, Duration::days(30));
    assert!(
        bench
            .due(the_period() - Duration::seconds(1))
            .await
            .is_empty(),
        "not due before its thirty days are over"
    );
    assert_eq!(bench.due(the_period()).await, [a_project_trashed(&id)]);
    let in_the_trash = bench.in_the_trash().await;
    assert_eq!(in_the_trash.len(), 1);
    assert_eq!(in_the_trash[0].trashed_until, Some(the_period()));
    assert!(
        bench.listed().await.is_empty(),
        "a project in the trash is not listed"
    );
}

#[tokio::test]
async fn a_project_in_the_trash_is_read_by_nobody() {
    let bench = bench();
    let id = bench.a_project("Tapestry").await;

    bench.trashed(&id).await;

    assert!(matches!(
        bench.wired.projects.read(&id, &local()).await,
        Err(ProjectsError::Events(ServiceError::NotFound { .. }))
    ));
}

#[tokio::test]
async fn trashing_a_project_twice_keeps_it_until_the_first_time_said() {
    let bench = bench();
    let id = bench.a_project("Tapestry").await;
    bench.trashed(&id).await;
    bench.clock.set(at(1_000) + Duration::days(10));

    bench.trashed(&id).await;

    assert_eq!(
        bench.due(the_period()).await,
        [a_project_trashed(&id)],
        "a second trashing is no failure and buys no time"
    );
}

#[tokio::test]
async fn a_trashing_that_does_not_commit_leaves_neither_the_event_nor_the_entry() {
    let store = Arc::new(InMemoryEventStore::new());
    let trash = Arc::new(InMemoryTrash::new());
    let core_on = |transactions: InMemoryTransactions| {
        ProjectsCore::new(
            store.clone(),
            Arc::new(transactions),
            Arc::new(InMemoryProjectCatalog::new()),
            trash.clone(),
            Arc::new(AllowingEverything),
            Arc::new(FixedClock::new(at(1_000))),
        )
    };
    let (id, _) = core_on(InMemoryTransactions::new())
        .start("Tapestry", &RawDetails::default(), &local())
        .await
        .expect("starting should succeed");

    core_on(InMemoryTransactions::refusing_commits())
        .trash(&id.to_string(), None, &local())
        .await
        .expect_err("a transaction that does not commit fails the trashing");

    assert!(
        trash
            .due(the_period(), 100)
            .await
            .expect("asking")
            .is_empty()
    );
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
    let bench = bench();
    let id = bench.a_project("Tapestry").await;
    bench.trashed(&id).await;

    bench
        .wired
        .projects
        .restore(&id, &local())
        .await
        .expect("restoring should succeed");
    bench.wired.settle().await;

    assert!(bench.due(the_period()).await.is_empty());
    assert!(bench.in_the_trash().await.is_empty());
    assert_eq!(bench.listed().await.len(), 1);
    bench
        .wired
        .projects
        .rename(&id, "Rewoven", None, &local())
        .await
        .expect("a project restored takes changes again");
}

#[tokio::test]
async fn a_project_not_in_the_trash_cannot_be_restored() {
    let bench = bench();
    let id = bench.a_project("Tapestry").await;

    assert!(matches!(
        bench.wired.projects.restore(&id, &local()).await,
        Err(ProjectsError::Events(ServiceError::Refused(
            ProjectError::NotTrashed
        )))
    ));
}

#[tokio::test]
async fn a_project_whose_time_in_the_trash_is_over_is_purged_and_deleted() {
    let bench = bench();
    let id = bench.a_project("Tapestry").await;
    bench.trashed(&id).await;
    bench.clock.set(the_period() - Duration::seconds(1));
    assert_eq!(
        bench.wired.purged().await,
        0,
        "nothing purged before its time"
    );

    bench.clock.set(the_period());
    let purged = bench.wired.purged().await;

    assert_eq!(purged, 1);
    assert!(
        bench.due(the_period()).await.is_empty(),
        "taken out with the purge"
    );
    assert!(
        bench.in_the_trash().await.is_empty(),
        "the deletion was told and the list forgot the project"
    );
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
    let bench = bench();
    let id = bench.a_project("Tapestry").await;
    bench.trashed(&id).await;
    bench
        .wired
        .projects
        .restore(&id, &local())
        .await
        .expect("restoring should succeed");
    bench.clock.set(the_period() + Duration::days(1));

    assert_eq!(bench.wired.purged().await, 0);
    assert!(bench.wired.projects.read(&id, &local()).await.is_ok());
}

#[tokio::test]
async fn a_purge_of_a_project_restored_meanwhile_only_takes_its_entry_out() {
    let bench = bench();
    let id = bench.a_project("Tapestry").await;
    bench.trashed(&id).await;
    bench
        .wired
        .projects
        .restore(&id, &local())
        .await
        .expect("restoring should succeed");

    bench
        .wired
        .projects
        .purge(&id)
        .await
        .expect("a purge that finds the project restored is no failure");

    assert!(bench.wired.projects.read(&id, &local()).await.is_ok());
}

#[tokio::test]
async fn only_an_owner_trashes_restores_and_sees_the_trash() {
    let roles = Arc::new(Roles::default());
    let wired = wiring::wired_guarded_by(Arc::new(FixedClock::new(at(1_000))), roles.clone());
    let id = wired
        .projects
        .start("Shared", &RawDetails::default(), &an_account(ADA))
        .await
        .expect("starting should succeed")
        .0
        .to_string();
    roles.give(ADA, &id, &OWNER);
    roles.give(GRACE, &id, &AUTHOR);

    assert!(matches!(
        wired.projects.trash(&id, None, &an_account(GRACE)).await,
        Err(ProjectsError::NotAnOwner)
    ));
    wired
        .projects
        .trash(&id, None, &an_account(ADA))
        .await
        .expect("an owner trashes");
    wired.settle().await;
    roles.give(ADA, &id, &IN_THE_TRASH_AN_OWNER_MAY);

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
    assert!(matches!(
        wired.projects.restore(&id, &an_account(GRACE)).await,
        Err(ProjectsError::NotAnOwner)
    ));
    wired
        .projects
        .restore(&id, &an_account(ADA))
        .await
        .expect("an owner restores");
}

#[tokio::test]
async fn an_owner_changing_a_project_in_the_trash_hears_it_is_there_and_others_that_it_is_gone() {
    let roles = Arc::new(Roles::default());
    let wired = wiring::wired_guarded_by(Arc::new(FixedClock::new(at(1_000))), roles.clone());
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
    roles.give(GRACE, &id, &[]);

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
    assert!(
        matches!(
            wired.projects.read(&id, &an_account(ADA)).await,
            Err(ProjectsError::Events(ServiceError::NotFound { .. }))
        ),
        "a project in the trash is read by nobody, its owners too"
    );
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
    let bench = bench();
    let id = bench.a_project("Tapestry").await;
    let server = server_of(&bench.wired);

    server
        .delete(&format!("/projects/{id}"))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    bench.wired.settle().await;
    server
        .get(&format!("/projects/{id}"))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    let in_the_trash: Vec<TrashedProjectDTO> = server.get("/projects/trash").await.json();
    assert_eq!(in_the_trash.len(), 1);
    assert_eq!(in_the_trash[0].id, id);
    assert_eq!(in_the_trash[0].name, "Tapestry");
    assert_eq!(in_the_trash[0].until, clock::text::serialize(the_period()));

    server
        .post(&format!("/projects/{id}/restore"))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    let again = server.post(&format!("/projects/{id}/restore")).await;
    again.assert_status(StatusCode::CONFLICT);
    assert_eq!(again.json::<Value>()["type"], refused::NOT_TRASHED);
    server
        .get(&format!("/projects/{id}"))
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn restoring_a_project_nobody_started_is_not_found() {
    let bench = bench();
    let server = server_of(&bench.wired);

    server
        .post("/projects/project_0000000000000000000000/restore")
        .await
        .assert_status(StatusCode::NOT_FOUND);
}
