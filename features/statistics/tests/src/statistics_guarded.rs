use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use access::{AccessCatalog, AccessError, HearsAccess, InMemoryAccess, KeptAccess, Role};
use async_trait::async_trait;
use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, Version};
use live::Broadcaster;
use live::testing::Subscriber;
use serde_json::Value;
use statistics_catalog::InMemoryWordCountCatalog;
use statistics_core::StatisticsCore;
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transaction};

const PROJECT: &str = "project_1";

/// Who may do what as the feature heard it, which can be made unreachable.
#[derive(Default)]
struct Reachable {
    heard: InMemoryAccess,
    down: AtomicBool,
}

impl Reachable {
    fn reached(&self) -> Result<(), AccessError> {
        if self.down.load(Ordering::SeqCst) {
            return Err(AccessError::Backend("the list is down".into()));
        }

        Ok(())
    }
}

#[async_trait]
impl AccessCatalog for Reachable {
    async fn remember_role(
        &self,
        transaction: &mut Transaction,
        project: &str,
        account: &AccountId,
        version: Version,
        role: Option<Role>,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.heard
            .remember_role(transaction, project, account, version, role, at)
            .await
    }

    async fn remember_disbanded(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.heard
            .remember_disbanded(transaction, project, version, at)
            .await
    }

    async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), AccessError> {
        self.heard
            .remember_trash(transaction, project, version, in_the_trash)
            .await
    }

    async fn remember_operator(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        version: Version,
        operator: bool,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.heard
            .remember_operator(transaction, account, version, operator, at)
            .await
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, AccessError> {
        self.heard
            .forget_tombstones_laid_before(transaction, before, at_most)
            .await
    }

    async fn role_of(
        &self,
        project: &str,
        account: &AccountId,
    ) -> Result<Option<Role>, AccessError> {
        self.reached()?;
        self.heard.role_of(project, account).await
    }

    async fn is_in_the_trash(&self, project: &str) -> Result<bool, AccessError> {
        self.reached()?;
        self.heard.is_in_the_trash(project).await
    }

    async fn projects_of(&self, account: &AccountId) -> Result<Vec<String>, AccessError> {
        self.reached()?;
        self.heard.projects_of(account).await
    }

    async fn share_a_project(
        &self,
        one: &AccountId,
        other: &AccountId,
    ) -> Result<bool, AccessError> {
        self.reached()?;
        self.heard.share_a_project(one, other).await
    }

    async fn operates(&self, account: &AccountId) -> Result<bool, AccessError> {
        self.reached()?;
        self.heard.operates(account).await
    }
}

struct Guarded {
    access: Arc<Reachable>,
    statistics: StatisticsCore,
    ada: TestServer,
    grace: TestServer,
}

async fn a_project_only_ada_may_read() -> Guarded {
    let access = Arc::new(Reachable::default());
    let kept_access = KeptAccess::new(access.clone());
    let statistics = StatisticsCore::new(
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryWordCountCatalog::new()),
        kept_access.clone(),
        Arc::new(kept_access),
        Arc::new(FixedClock::new(clock::at(1_700_000_000))),
    );
    statistics
        .remember_role(
            None,
            PROJECT,
            &AccountId::from("account_ada"),
            Version::of(2),
            Some(Role::Reader),
        )
        .await
        .expect("remembering should succeed");
    statistics
        .counted(None, "scene_1", PROJECT, 42, OffsetDateTime::UNIX_EPOCH)
        .await
        .expect("keeping should succeed");
    let routes = statistics_rest::router(statistics.clone()).merge(live::live_socket(
        live::Topics::of(vec![statistics_rest::topic(
            statistics.clone(),
            Broadcaster::new(),
        )]),
    ));
    let as_account = |account: &str| {
        TestServer::builder().http_transport().build(
            routes
                .clone()
                .layer(Extension(Agent::Account(AccountId::from(account)))),
        )
    };

    Guarded {
        ada: as_account("account_ada"),
        grace: as_account("account_grace"),
        access,
        statistics,
    }
}

async fn following(account: &TestServer) -> Subscriber {
    let mut socket = Subscriber::on(account).await;
    socket
        .subscribe("counts", "word-counts", &[("project", PROJECT)])
        .await;

    socket
}

#[tokio::test]
async fn whoever_may_read_the_project_is_told_its_counts_and_follows_them() {
    // given: a counted project only ada may read
    let guarded = a_project_only_ada_may_read().await;

    // when: ada asks for its counts
    let counts = guarded
        .ada
        .get(&format!("/statistics/word-counts?project={PROJECT}"))
        .await;

    // then: she is told them
    counts.assert_status_ok();
    assert_eq!(counts.json::<Value>()[0]["words"], 42);
    // and: she may follow them
    following(&guarded.ada).await.nothing().await;
}

#[tokio::test]
async fn a_project_s_counts_are_not_there_for_whoever_may_not_read_it() {
    // given: a counted project only ada may read
    let guarded = a_project_only_ada_may_read().await;

    // when: grace asks for its counts
    let counts = guarded
        .grace
        .get(&format!("/statistics/word-counts?project={PROJECT}"))
        .await;

    // then: they are not there
    counts.assert_status(StatusCode::NOT_FOUND);
    assert_eq!(counts.json::<Value>()["type"], "not-found");
    // and: following them finds nothing there
    assert_eq!(following(&guarded.grace).await.ended().await, "not-there");
}

#[tokio::test]
async fn whom_to_let_in_not_being_known_is_a_failure_not_a_no() {
    // given: a counted project only ada may read
    let guarded = a_project_only_ada_may_read().await;
    // and: who may read it cannot be known
    guarded.access.down.store(true, Ordering::SeqCst);

    // then: ada asking for its counts fails
    guarded
        .ada
        .get(&format!("/statistics/word-counts?project={PROJECT}"))
        .await
        .assert_status(StatusCode::INTERNAL_SERVER_ERROR);
    // and: so does her following them
    assert_eq!(following(&guarded.ada).await.ended().await, "failed");
}

#[tokio::test]
async fn a_follower_who_may_no_longer_read_the_project_is_closed_out() {
    // given: a counted project only ada may read
    let guarded = a_project_only_ada_may_read().await;
    // and: ada following its counts
    let mut socket = following(&guarded.ada).await;

    // when: ada may no longer read it
    guarded
        .statistics
        .remember_role(
            None,
            PROJECT,
            &AccountId::from("account_ada"),
            Version::of(3),
            None,
        )
        .await
        .expect("remembering should succeed");

    // then: she is closed out
    assert_eq!(socket.ended().await, "access-lost");
}
