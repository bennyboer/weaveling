use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use authorizing::{Act, Authorizer, AuthorizerError, Readable};
use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use eventsourcing::{AccountId, Agent};
use live::Broadcaster;
use serde_json::Value;
use statistics_catalog::InMemoryWordCountCatalog;
use statistics_core::{ProjectLink, SceneLink, WordCountCatalog};
use time::OffsetDateTime;

const PROJECT: &str = "project_1";

#[derive(Default)]
struct OnlyAda {
    down: Mutex<bool>,
}

#[async_trait]
impl Authorizer for OnlyAda {
    async fn may(&self, agent: &Agent, act: Act, project: &str) -> Result<bool, AuthorizerError> {
        if *self.down.lock().expect("never poisoned") {
            return Err(AuthorizerError::Unreachable("the list is down".into()));
        }

        Ok(act == Act::Read
            && project == PROJECT
            && *agent == Agent::Account(AccountId::from("account_ada")))
    }

    async fn readable_by(&self, _agent: &Agent) -> Result<Readable, AuthorizerError> {
        Ok(Readable::Everything)
    }
}

struct Guarded {
    authorizer: Arc<OnlyAda>,
    ada: TestServer,
    grace: TestServer,
}

async fn guarded() -> Guarded {
    let authorizer = Arc::new(OnlyAda::default());
    let catalog = Arc::new(InMemoryWordCountCatalog::new());
    catalog
        .keep(
            &SceneLink::from("scene_1"),
            &ProjectLink::from(PROJECT),
            42,
            OffsetDateTime::UNIX_EPOCH,
        )
        .await
        .expect("keeping should succeed");
    let routes = statistics_rest::router(catalog, authorizer.clone()).merge(statistics_rest::live(
        Broadcaster::new(),
        authorizer.clone(),
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

#[tokio::test]
async fn whoever_may_read_the_project_is_told_its_counts_and_follows_them() {
    let guarded = guarded().await;

    let counts = guarded
        .ada
        .get(&format!("/statistics/word-counts?project={PROJECT}"))
        .await;

    counts.assert_status_ok();
    assert_eq!(counts.json::<Value>()[0]["words"], 42);
    guarded
        .ada
        .get_websocket(&format!("/statistics/word-counts/live?project={PROJECT}"))
        .await
        .assert_status(StatusCode::SWITCHING_PROTOCOLS);
}

#[tokio::test]
async fn a_project_s_counts_are_not_there_for_whoever_may_not_read_it() {
    let guarded = guarded().await;

    let counts = guarded
        .grace
        .get(&format!("/statistics/word-counts?project={PROJECT}"))
        .await;

    counts.assert_status(StatusCode::NOT_FOUND);
    assert_eq!(counts.json::<Value>()["type"], "not-found");
    guarded
        .grace
        .get_websocket(&format!("/statistics/word-counts/live?project={PROJECT}"))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn whom_to_let_in_not_being_known_is_a_failure_not_a_no() {
    let guarded = guarded().await;
    *guarded.authorizer.down.lock().expect("never poisoned") = true;

    guarded
        .ada
        .get(&format!("/statistics/word-counts?project={PROJECT}"))
        .await
        .assert_status(StatusCode::INTERNAL_SERVER_ERROR);
}
