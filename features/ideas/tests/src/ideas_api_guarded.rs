use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use access::{AccessCatalog, AccessError, HearsAccess, InMemoryAccess, Role};
use async_trait::async_trait;
use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, Version};
use live::testing::Subscriber;
use live_contract::{ACCESS_CHANGED, ToldDTO};
use serde_json::{Value, json};
use time::OffsetDateTime;
use transacting::Transaction;

use crate::wiring;
use crate::wiring::Wired;

const PROJECT: &str = "project_1";

/// Who may do what, kept in memory, that can be made unreachable.
#[derive(Default)]
struct Faltering {
    kept: InMemoryAccess,
    down: AtomicBool,
}

impl Faltering {
    fn reachable(&self) -> Result<(), AccessError> {
        if self.down.load(Ordering::Acquire) {
            return Err(AccessError::Backend("the list is down".into()));
        }

        Ok(())
    }
}

#[async_trait]
impl AccessCatalog for Faltering {
    async fn remember_role(
        &self,
        transaction: &mut Transaction,
        project: &str,
        account: &AccountId,
        version: Version,
        role: Option<Role>,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.kept
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
        self.kept
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
        self.kept
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
        self.kept
            .remember_operator(transaction, account, version, operator, at)
            .await
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, AccessError> {
        self.kept
            .forget_tombstones_laid_before(transaction, before, at_most)
            .await
    }

    async fn role_of(
        &self,
        project: &str,
        account: &AccountId,
    ) -> Result<Option<Role>, AccessError> {
        self.reachable()?;
        self.kept.role_of(project, account).await
    }

    async fn is_in_the_trash(&self, project: &str) -> Result<bool, AccessError> {
        self.reachable()?;
        self.kept.is_in_the_trash(project).await
    }

    async fn projects_of(&self, account: &AccountId) -> Result<Vec<String>, AccessError> {
        self.reachable()?;
        self.kept.projects_of(account).await
    }

    async fn share_a_project(
        &self,
        one: &AccountId,
        other: &AccountId,
    ) -> Result<bool, AccessError> {
        self.reachable()?;
        self.kept.share_a_project(one, other).await
    }

    async fn operates(&self, account: &AccountId) -> Result<bool, AccessError> {
        self.reachable()?;
        self.kept.operates(account).await
    }

    async fn owners_of(&self, project: &str) -> Result<Vec<AccountId>, AccessError> {
        self.reachable()?;
        self.kept.owners_of(project).await
    }
}

struct Guarded {
    wired: Wired,
    access: Arc<Faltering>,
    told: AtomicU64,
    ada: TestServer,
    grace: TestServer,
    idea: String,
}

impl Guarded {
    async fn give(&self, account: &str, role: Option<Role>) {
        self.wired
            .ideas
            .remember_role(
                None,
                PROJECT,
                &AccountId::from(account),
                Version::of(self.told.fetch_add(1, Ordering::AcqRel) + 1),
                role,
            )
            .await
            .expect("remembering a role should succeed");
    }
}

fn as_account(wired: &Wired, account: &str) -> TestServer {
    TestServer::builder().http_transport().build(
        wired
            .routes
            .clone()
            .layer(Extension(Agent::Account(AccountId::from(account)))),
    )
}

async fn an_idea_in_ada_s_project() -> Guarded {
    let access = Arc::new(Faltering::default());
    let wired = wiring::wired_keeping(
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        access.clone(),
        true,
    );
    let guarded = Guarded {
        grace: as_account(&wired, "account_grace"),
        ada: as_account(&wired, "account_ada"),
        access,
        told: AtomicU64::new(0),
        wired,
        idea: String::new(),
    };
    guarded.give("account_ada", Some(Role::Owner)).await;
    let captured = guarded
        .ada
        .post("/ideas")
        .json(&json!({ "project": PROJECT, "title": "A girl in a wood" }))
        .await;
    captured.assert_status(StatusCode::CREATED);
    let idea = captured.json::<Value>()["id"]
        .as_str()
        .expect("an idea has an id")
        .to_owned();
    guarded.wired.settle().await;

    Guarded { idea, ..guarded }
}

async fn title_of(guarded: &Guarded) -> Value {
    guarded
        .ada
        .get(&format!("/ideas/{}", guarded.idea))
        .await
        .json::<Value>()["title"]
        .clone()
}

#[tokio::test]
async fn the_ideas_of_a_project_are_not_there_for_whoever_may_not_read_it() {
    // given: an idea in a project grace may not read
    let guarded = an_idea_in_ada_s_project().await;
    let at = format!("/ideas/{}", guarded.idea);

    // then: every request of hers about its ideas is not found
    for answered in [
        guarded
            .grace
            .get(&format!("/ideas?project={PROJECT}"))
            .await,
        guarded
            .grace
            .post("/ideas")
            .json(&json!({ "project": PROJECT, "title": "Theirs" }))
            .await,
        guarded.grace.get(&at).await,
        guarded
            .grace
            .patch(&at)
            .json(&json!({ "title": "Theirs" }))
            .await,
        guarded.grace.delete(&at).await,
    ] {
        answered.assert_status(StatusCode::NOT_FOUND);
        assert_eq!(answered.json::<Value>()["type"], "not-found");
    }
    // and: following them is not there
    let mut hers = Subscriber::on(&guarded.grace).await;
    hers.subscribe("ideas", "ideas", &[("project", PROJECT)])
        .await;
    assert_eq!(hers.ended().await, "not-there");
    // and: the idea stays as it was
    assert_eq!(title_of(&guarded).await, "A girl in a wood");
}

#[tokio::test]
async fn a_reader_reads_and_follows_the_ideas_and_is_told_only_an_author_may_change_them() {
    // given: an idea in a project grace may read
    let guarded = an_idea_in_ada_s_project().await;
    guarded.give("account_grace", Some(Role::Reader)).await;
    let at = format!("/ideas/{}", guarded.idea);

    // then: she can list and fetch its ideas
    let listed = guarded
        .grace
        .get(&format!("/ideas?project={PROJECT}"))
        .await;
    listed.assert_status_ok();
    assert_eq!(listed.json::<Value>()[0]["id"], guarded.idea.as_str());
    guarded.grace.get(&at).await.assert_status_ok();
    // and: follow them
    let mut hers = Subscriber::on(&guarded.grace).await;
    hers.subscribe("ideas", "ideas", &[("project", PROJECT)])
        .await;
    hers.nothing().await;
    // and: every change she asks for is refused as not an author's
    for refused in [
        guarded
            .grace
            .post("/ideas")
            .json(&json!({ "project": PROJECT, "title": "Theirs" }))
            .await,
        guarded
            .grace
            .patch(&at)
            .json(&json!({ "title": "Theirs" }))
            .await,
        guarded.grace.delete(&at).await,
    ] {
        refused.assert_status(StatusCode::FORBIDDEN);
        assert_eq!(refused.json::<Value>()["type"], "not-an-author");
    }
    // and: the idea stays as it was
    assert_eq!(title_of(&guarded).await, "A girl in a wood");
}

#[tokio::test]
async fn an_author_captures_retitles_and_discards() {
    // given: an idea in a project grace may write in
    let guarded = an_idea_in_ada_s_project().await;
    guarded.give("account_grace", Some(Role::Author)).await;
    let at = format!("/ideas/{}", guarded.idea);

    // then: she may capture an idea
    guarded
        .grace
        .post("/ideas")
        .json(&json!({ "project": PROJECT, "title": "Hers" }))
        .await
        .assert_status(StatusCode::CREATED);
    // and: retitle one
    guarded
        .grace
        .patch(&at)
        .json(&json!({ "title": "A boy in a wood" }))
        .await
        .assert_status_ok();
    assert_eq!(title_of(&guarded).await, "A boy in a wood");
    // and: discard one
    guarded
        .grace
        .delete(&at)
        .await
        .assert_status(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn a_follower_whose_role_is_taken_away_is_closed_out_and_the_others_follow_on() {
    // given: grace and the owner following a project's ideas
    let guarded = an_idea_in_ada_s_project().await;
    guarded.give("account_grace", Some(Role::Reader)).await;
    let mut hers = Subscriber::on(&guarded.grace).await;
    hers.subscribe("ideas", "ideas", &[("project", PROJECT)])
        .await;
    let mut ada_s = Subscriber::on(&guarded.ada).await;
    ada_s
        .subscribe("ideas", "ideas", &[("project", PROJECT)])
        .await;

    // when: grace's role is taken away
    guarded.give("account_grace", None).await;

    // then: she is closed out
    assert_eq!(
        hers.ended().await,
        "access-lost",
        "a member removed hears nothing more of the project"
    );
    // and: the owner is told the members changed
    assert!(
        matches!(ada_s.told().await, ToldDTO::Notice { notice, .. } if notice == ACCESS_CHANGED),
        "the owner stays and is told the members changed"
    );
    // when: the owner captures an idea
    guarded
        .ada
        .post("/ideas")
        .json(&json!({ "project": PROJECT, "title": "Told to the owner" }))
        .await
        .assert_status(StatusCode::CREATED);
    guarded.wired.settle().await;
    // then: the owner is told of it
    assert_eq!(
        ada_s.frame().await["event"]["title"],
        "Told to the owner",
        "the owner still follows"
    );
}

#[tokio::test]
async fn whom_to_let_in_not_being_known_is_a_failure_not_a_no() {
    // given: an idea
    let guarded = an_idea_in_ada_s_project().await;
    // and: whom to let in not known
    guarded.access.down.store(true, Ordering::Release);

    // then: reading its ideas fails
    for answered in [
        guarded.ada.get(&format!("/ideas?project={PROJECT}")).await,
        guarded.ada.get(&format!("/ideas/{}", guarded.idea)).await,
    ] {
        answered.assert_status(StatusCode::INTERNAL_SERVER_ERROR);
    }
    // and: following them fails
    let mut ada_s = Subscriber::on(&guarded.ada).await;
    ada_s
        .subscribe("ideas", "ideas", &[("project", PROJECT)])
        .await;
    assert_eq!(ada_s.ended().await, "failed");
}
