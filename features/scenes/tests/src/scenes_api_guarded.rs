use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use authorizing::{AccessChanges, AccessWatch, Act, Authorizer, AuthorizerError, Readable};
use axum::Extension;
use axum::http::StatusCode;
use axum_test::{TestResponse, TestServer};
use belonging::InMemoryBelongings;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use live::Broadcaster;
use live::testing::Subscriber;
use scenes_catalog::{InMemoryQuotationRestylingTasks, InMemorySceneCatalog};
use scenes_core::ScenesCore;
use scenes_store::InMemorySceneStore;
use serde_json::{Value, json};
use time::OffsetDateTime;
use transacting::InMemoryTransactions;

const PROJECT: &str = "project_1";
const OWNER: [Act; 3] = [Act::Read, Act::Write, Act::Manage];
const READER: [Act; 1] = [Act::Read];

#[derive(Default)]
struct Roles {
    held: Mutex<HashMap<String, Vec<Act>>>,
    down: Mutex<bool>,
    changes: AccessChanges,
}

impl Roles {
    fn give(&self, account: &str, acts: &[Act]) {
        self.held
            .lock()
            .expect("never poisoned")
            .insert(account.to_owned(), acts.to_vec());
    }
}

#[async_trait]
impl Authorizer for Roles {
    async fn may(&self, agent: &Agent, act: Act, project: &str) -> Result<bool, AuthorizerError> {
        if *self.down.lock().expect("never poisoned") {
            return Err(AuthorizerError::Unreachable("the list is down".into()));
        }
        let Agent::Account(account) = agent else {
            return Ok(false);
        };

        Ok(project == PROJECT
            && self
                .held
                .lock()
                .expect("never poisoned")
                .get(account.as_str())
                .is_some_and(|acts| acts.contains(&act)))
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

struct Wired {
    roles: Arc<Roles>,
    ada: TestServer,
    grace: TestServer,
    scene: String,
}

async fn a_workbench() -> Wired {
    let roles = Arc::new(Roles::default());
    roles.give("account_ada", &OWNER);
    let scenes = ScenesCore::new(
        Arc::new(InMemorySceneStore::new()),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemorySceneCatalog::new()),
        Arc::new(InMemoryQuotationRestylingTasks::new()),
        Arc::new(InMemoryBelongings::new()),
        roles.clone(),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );
    let routes = scenes_rest::router(scenes.clone())
        .merge(scenes_rest::quotation_restylings(scenes.clone()))
        .merge(live::live_socket(live::Topics::of(vec![
            scenes_rest::scenes_topic(scenes.clone(), Broadcaster::new()),
            scenes_rest::restylings_topic(scenes, Broadcaster::new()),
        ])));
    let as_account = |account: &str| {
        TestServer::builder().http_transport().build(
            routes
                .clone()
                .layer(Extension(Agent::Account(AccountId::from(account)))),
        )
    };
    let ada = as_account("account_ada");
    let created = ada
        .post("/scenes")
        .json(&json!({ "project": PROJECT }))
        .await;
    created.assert_status(StatusCode::CREATED);
    let scene = created.json::<Value>()["id"]
        .as_str()
        .expect("a scene has an id")
        .to_owned();
    ada.post(&format!("/scenes/{scene}/ideas"))
        .json(&json!({ "idea": "idea_1" }))
        .await
        .assert_status_ok();

    Wired {
        grace: as_account("account_grace"),
        ada,
        roles,
        scene,
    }
}

async fn the_title(wired: &Wired) -> Value {
    wired
        .ada
        .get(&format!("/scenes/{}", wired.scene))
        .await
        .json::<Value>()["title"]
        .clone()
}

async fn changes_by(wired: &Wired, account: &TestServer) -> [TestResponse; 5] {
    let at = format!("/scenes/{}", wired.scene);

    [
        account
            .post("/scenes")
            .json(&json!({ "project": PROJECT }))
            .await,
        account.patch(&at).json(&json!({ "title": "Theirs" })).await,
        account
            .post(&format!("{at}/ideas"))
            .json(&json!({ "idea": "idea_2" }))
            .await,
        account.delete(&format!("{at}/ideas/idea_1")).await,
        account.delete(&at).await,
    ]
}

async fn reads_by(wired: &Wired, account: &TestServer) -> [TestResponse; 3] {
    [
        account.get(&format!("/scenes?project={PROJECT}")).await,
        account.get(&format!("/scenes/{}", wired.scene)).await,
        account
            .get(&format!("/scenes/quotation-restylings/{PROJECT}"))
            .await,
    ]
}

const FOLLOWED: [&str; 2] = ["scenes", "quotation-restylings"];

async fn following(account: &TestServer) -> Subscriber {
    let mut socket = Subscriber::on(account).await;
    for topic in FOLLOWED {
        socket
            .subscribe(topic, topic, &[("project", PROJECT)])
            .await;
    }

    socket
}

#[tokio::test]
async fn a_project_s_scenes_are_not_there_for_whoever_may_not_read_it() {
    // given: a project with a scene, and an account with no role in it
    let wired = a_workbench().await;

    // when: the account reads and changes its scenes
    let mut answered = Vec::from(reads_by(&wired, &wired.grace).await);
    answered.extend(changes_by(&wired, &wired.grace).await);

    // then: every answer is that they are not there
    for answer in answered {
        answer.assert_status(StatusCode::NOT_FOUND);
        assert_eq!(answer.json::<Value>()["type"], "not-found");
    }
    // and: following them ends as not there
    let mut hers = following(&wired.grace).await;
    for _ in FOLLOWED {
        assert_eq!(hers.ended().await, "not-there");
    }
    // and: the scene is unchanged
    assert_eq!(the_title(&wired).await, "");
}

#[tokio::test]
async fn a_reader_reads_and_follows_the_scenes_and_is_told_only_an_author_may_change_them() {
    // given: a project with a scene, and a reader of it
    let wired = a_workbench().await;
    wired.roles.give("account_grace", &READER);

    // when: the reader reads its scenes
    let [listed, read, restyled] = reads_by(&wired, &wired.grace).await;

    // then: each is answered
    listed.assert_status_ok();
    assert_eq!(
        listed.json::<Value>()["scenes"].as_array().map(Vec::len),
        Some(0),
        "the list is the catalog's, which no listener filled here"
    );
    read.assert_status_ok();
    restyled.assert_status(StatusCode::NOT_FOUND);
    assert_eq!(
        restyled.json::<Value>()["type"],
        "quotation-never-restyled",
        "a reader is told what there is, not that the project is not there"
    );
    // and: following them is not refused
    following(&wired.grace).await.nothing().await;

    // when: the reader changes them
    let refusals = changes_by(&wired, &wired.grace).await;

    // then: each change is refused as needing an author
    for refused in refusals {
        refused.assert_status(StatusCode::FORBIDDEN);
        assert_eq!(refused.json::<Value>()["type"], "not-an-author");
    }
    // and: the scene is unchanged
    assert_eq!(the_title(&wired).await, "");
}

#[tokio::test]
async fn an_author_creates_retitles_links_unlinks_and_deletes() {
    // given: a project with a scene, and an author of it
    let wired = a_workbench().await;
    wired.roles.give("account_grace", &[Act::Read, Act::Write]);

    // when: the author changes its scenes
    let [created, retitled, linked, unlinked, deleted] = changes_by(&wired, &wired.grace).await;

    // then: every change goes through
    created.assert_status(StatusCode::CREATED);
    retitled.assert_status_ok();
    linked.assert_status_ok();
    unlinked.assert_status_ok();
    deleted.assert_status(StatusCode::NO_CONTENT);
    // and: the scene is gone
    wired
        .ada
        .get(&format!("/scenes/{}", wired.scene))
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn whom_to_let_in_not_being_known_is_a_failure_not_a_no() {
    // given: a project whose roles cannot be learned
    let wired = a_workbench().await;
    *wired.roles.down.lock().expect("never poisoned") = true;

    // then: its owner's reads fail
    for answered in reads_by(&wired, &wired.ada).await {
        answered.assert_status(StatusCode::INTERNAL_SERVER_ERROR);
    }
    // and: following its scenes ends as failed
    let mut ada_s = following(&wired.ada).await;
    for _ in FOLLOWED {
        assert_eq!(ada_s.ended().await, "failed");
    }
}

#[tokio::test]
async fn a_follower_whose_role_is_taken_away_is_closed_out() {
    // given: a reader following a project's scenes
    let wired = a_workbench().await;
    wired.roles.give("account_grace", &READER);
    let mut hers = following(&wired.grace).await;

    // when: her role is taken away
    wired.roles.give("account_grace", &[]);
    wired.roles.access_changed(PROJECT);

    // then: each following ends as access lost
    for _ in FOLLOWED {
        assert_eq!(hers.ended().await, "access-lost");
    }
}
