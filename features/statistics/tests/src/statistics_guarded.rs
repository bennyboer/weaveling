use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use authorizing::{AccessChanges, AccessWatch, Act, Authorizer, AuthorizerError, Readable};
use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use eventsourcing::{AccountId, Agent};
use live::Broadcaster;
use live::testing::Subscriber;
use serde_json::Value;
use statistics_catalog::InMemoryWordCountCatalog;
use statistics_core::StatisticsCore;
use time::OffsetDateTime;
use transacting::InMemoryTransactions;

const PROJECT: &str = "project_1";

#[derive(Default)]
struct OnlyAda {
    down: Mutex<bool>,
    ada_left: Mutex<bool>,
    changes: AccessChanges,
}

#[async_trait]
impl Authorizer for OnlyAda {
    async fn may(&self, agent: &Agent, act: Act, project: &str) -> Result<bool, AuthorizerError> {
        if *self.down.lock().expect("never poisoned") {
            return Err(AuthorizerError::Unreachable("the list is down".into()));
        }

        Ok(act == Act::Read
            && project == PROJECT
            && *agent == Agent::Account(AccountId::from("account_ada"))
            && !*self.ada_left.lock().expect("never poisoned"))
    }

    async fn readable_by(&self, _agent: &Agent) -> Result<Readable, AuthorizerError> {
        Ok(Readable::Everything)
    }

    fn access_changed(&self, project: &str) {
        self.changes.tell(project);
    }

    fn watch_access(&self, project: &str) -> AccessWatch {
        self.changes.watch(project)
    }
}

struct Guarded {
    authorizer: Arc<OnlyAda>,
    ada: TestServer,
    grace: TestServer,
}

async fn a_project_only_ada_may_read() -> Guarded {
    let authorizer = Arc::new(OnlyAda::default());
    let statistics = StatisticsCore::new(
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryWordCountCatalog::new()),
        authorizer.clone(),
    );
    statistics
        .counted(None, "scene_1", PROJECT, 42, OffsetDateTime::UNIX_EPOCH)
        .await
        .expect("keeping should succeed");
    let routes = statistics_rest::router(statistics.clone()).merge(live::live_socket(
        live::Topics::of(vec![statistics_rest::topic(statistics, Broadcaster::new())]),
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
        authorizer,
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
    *guarded.authorizer.down.lock().expect("never poisoned") = true;

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
    *guarded.authorizer.ada_left.lock().expect("never poisoned") = true;
    guarded.authorizer.access_changed(PROJECT);

    // then: she is closed out
    assert_eq!(socket.ended().await, "access-lost");
}
