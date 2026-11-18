use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::sample::SampleBook;
use access::{AccessCatalog, AccessError, HearsAccess, InMemoryAccess, KeptAccess, Role};
use async_trait::async_trait;
use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, Version};
use readings_core::ReadingsCore;
use serde_json::Value;
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transaction};

const PROJECT: &str = "project_loom";

/// Who may do what, as the feature heard it.
/// It can be made unreachable.
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

    async fn owners_of(&self, project: &str) -> Result<Vec<AccountId>, AccessError> {
        self.reached()?;
        self.heard.owners_of(project).await
    }
}

struct Guarded {
    access: Arc<Reachable>,
    readings: ReadingsCore,
    ada: TestServer,
    grace: TestServer,
}

async fn a_project_only_ada_may_read() -> Guarded {
    let access = Arc::new(Reachable::default());
    let kept_access = KeptAccess::new(access.clone());
    let readings = ReadingsCore::new(
        Arc::new(SampleBook),
        Arc::new(SampleBook),
        Arc::new(InMemoryTransactions::new()),
        kept_access.clone(),
        Arc::new(kept_access),
        Arc::new(FixedClock::new(clock::at(1_700_000_000))),
    );
    readings
        .remember_role(
            None,
            PROJECT,
            &AccountId::from("account_ada"),
            Version::of(2),
            Some(Role::Reader),
        )
        .await
        .expect("remembering should succeed");
    let routes = readings_rest::router(readings.clone());
    let as_account = |account: &str| {
        TestServer::new(
            routes
                .clone()
                .layer(Extension(Agent::Account(AccountId::from(account)))),
        )
    };

    Guarded {
        ada: as_account("account_ada"),
        grace: as_account("account_grace"),
        access,
        readings,
    }
}

#[tokio::test]
async fn whoever_may_read_the_project_is_shown_its_book() {
    // given: a project only ada may read
    let guarded = a_project_only_ada_may_read().await;

    // when: ada asks for its live book
    let book = guarded
        .ada
        .get(&format!("/readings/live?project={PROJECT}"))
        .await;

    // then: she is shown it
    book.assert_status_ok();
    assert_eq!(book.json::<Value>()["title"], "The Loom");
}

#[tokio::test]
async fn a_project_s_book_is_not_there_for_whoever_may_not_read_it() {
    // given: a project only ada may read
    let guarded = a_project_only_ada_may_read().await;

    // when: grace asks for its live book
    let book = guarded
        .grace
        .get(&format!("/readings/live?project={PROJECT}"))
        .expect_failure()
        .await;

    // then: it is not there, as for a project that does not exist
    book.assert_status(StatusCode::NOT_FOUND);
    assert_eq!(book.json::<Value>()["type"], "not-found");
}

#[tokio::test]
async fn a_project_in_the_trash_shows_its_book_to_nobody() {
    // given: a project only ada may read
    let guarded = a_project_only_ada_may_read().await;
    // and: the project put in the trash
    guarded
        .readings
        .remember_trash(None, PROJECT, Version::of(3), true)
        .await
        .expect("remembering should succeed");

    // when: ada asks for its live book
    let book = guarded
        .ada
        .get(&format!("/readings/live?project={PROJECT}"))
        .expect_failure()
        .await;

    // then: it is not there
    book.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn whom_to_let_in_not_being_known_is_a_failure_not_a_no() {
    // given: a project only ada may read
    let guarded = a_project_only_ada_may_read().await;
    // and: who may read it cannot be known
    guarded.access.down.store(true, Ordering::SeqCst);

    // when: ada asks for its live book
    let book = guarded
        .ada
        .get(&format!("/readings/live?project={PROJECT}"))
        .expect_failure()
        .await;

    // then: it fails
    book.assert_status(StatusCode::INTERNAL_SERVER_ERROR);
}
