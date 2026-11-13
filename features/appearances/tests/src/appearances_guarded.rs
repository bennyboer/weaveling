use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use access::{AccessCatalog, AccessError, HearsAccess, InMemoryAccess, KeptAccess, Role};
use appearances_catalog::InMemoryAppearanceCatalog;
use appearances_contract::PlaceDTO;
use appearances_core::{AppearancesCore, IdeaLink, Place, ProjectLink, SceneLink, Subject};
use appearances_rest::TellingAppearances;
use async_trait::async_trait;
use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, Version};
use live::Broadcaster;
use live::testing::Subscriber;
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transaction};

use crate::committed::Committed;

const READABLE: &str = "project_ada";
const UNREADABLE: &str = "project_kept";

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

    async fn owners_of(&self, project: &str) -> Result<Vec<AccountId>, AccessError> {
        self.reached()?;
        self.heard.owners_of(project).await
    }
}

struct Guarded {
    kept: Arc<InMemoryAppearanceCatalog>,
    telling: TellingAppearances,
    access: Arc<Reachable>,
    server: TestServer,
}

fn an_idea(id: &str) -> Subject {
    Subject::Idea(IdeaLink::from(id))
}

async fn guarded() -> Guarded {
    let broadcaster = Broadcaster::new();
    let kept = Arc::new(InMemoryAppearanceCatalog::new());
    let access = Arc::new(Reachable::default());
    let kept_access = KeptAccess::new(access.clone());
    let appearances = AppearancesCore::new(
        Arc::new(InMemoryTransactions::new()),
        kept.clone(),
        kept_access.clone(),
        Arc::new(kept_access),
        Arc::new(FixedClock::new(clock::at(1_700_000_000))),
    );
    appearances
        .remember_role(
            None,
            READABLE,
            &AccountId::from("account_ada"),
            Version::of(2),
            Some(Role::Reader),
        )
        .await
        .expect("remembering should succeed");
    let server = TestServer::builder().http_transport().build(
        appearances_rest::router(appearances.clone())
            .merge(live::live_socket(live::Topics::of(vec![Arc::new(
                appearances_rest::AppearancesTopic::new(appearances, broadcaster.clone()),
            )])))
            .layer(Extension(Agent::Account(AccountId::from("account_ada")))),
    );
    for (idea, project, scene) in [
        ("idea_ada", READABLE, "scene_1"),
        ("idea_kept", UNREADABLE, "scene_kept"),
    ] {
        kept.remember_project_committed(&an_idea(idea), &ProjectLink::from(project))
            .await
            .expect("remembering should succeed");
        kept.remember_committed(&an_idea(idea), &Place::Scene(SceneLink::from(scene)), 1)
            .await
            .expect("remembering should succeed");
    }

    Guarded {
        telling: TellingAppearances::around(kept.clone(), broadcaster),
        kept,
        access,
        server,
    }
}

async fn following(server: &TestServer, idea: &str) -> Subscriber {
    let mut socket = Subscriber::on(server).await;
    socket
        .subscribe("appearances", "appearances", &[("idea", idea)])
        .await;

    socket
}

async fn heard_soon(socket: &mut Subscriber) -> bool {
    !socket.silent().await
}

#[tokio::test]
async fn where_an_idea_of_a_project_one_may_read_appears_is_told_and_followed() {
    // given: a followed idea of a project one may read
    let guarded = guarded().await;
    let mut socket = following(&guarded.server, "idea_ada").await;

    // when: where it appears is asked
    let listed = guarded.server.get("/appearances?idea=idea_ada").await;
    // and: it is placed somewhere new
    guarded
        .telling
        .remember_committed(
            &an_idea("idea_ada"),
            &Place::Scene(SceneLink::from("scene_2")),
            2,
        )
        .await
        .expect("remembering should succeed");

    // then: the places it had are told
    listed.assert_status_ok();
    assert_eq!(
        listed.json::<Vec<PlaceDTO>>(),
        vec![PlaceDTO::Scene {
            id: "scene_1".to_owned()
        }]
    );
    // and: its follower hears of the change
    assert_eq!(socket.frame().await["of"], "idea_ada");
}

#[tokio::test]
async fn an_idea_of_a_project_one_may_not_read_is_not_there() {
    // given: an idea of a project one may not read
    let guarded = guarded().await;

    // then: asking where it appears finds nothing
    guarded
        .server
        .get("/appearances?idea=idea_kept")
        .await
        .assert_status(StatusCode::NOT_FOUND);
    // and: following it ends as not there
    assert_eq!(
        following(&guarded.server, "idea_kept").await.ended().await,
        "not-there"
    );
}

#[tokio::test]
async fn an_idea_of_no_project_known_yet_appears_nowhere_and_is_told_only_once_it_may_be_read() {
    // given: ideas of no project known yet
    let guarded = guarded().await;
    // when: where one of them appears is asked
    let listed = guarded.server.get("/appearances?idea=idea_new").await;
    // and: each is followed
    let mut kept_from = following(&guarded.server, "idea_new").await;
    let mut readable = following(&guarded.server, "idea_later").await;
    let mut orphaned = following(&guarded.server, "idea_orphan").await;

    // and: one turns out of a project kept from one, one of a readable project, and both are placed
    for (idea, project) in [("idea_new", UNREADABLE), ("idea_later", READABLE)] {
        guarded
            .kept
            .remember_project_committed(&an_idea(idea), &ProjectLink::from(project))
            .await
            .expect("remembering should succeed");
        guarded
            .telling
            .remember_committed(&an_idea(idea), &Place::Scene(SceneLink::from("scene_9")), 1)
            .await
            .expect("remembering should succeed");
    }

    // then: it appears nowhere
    listed.assert_status_ok();
    assert!(listed.json::<Vec<PlaceDTO>>().is_empty());
    // and: the one of the readable project is told
    assert!(
        heard_soon(&mut readable).await,
        "an idea whose project turns out readable is told, so the frames do go out"
    );
    // and: the one of the kept project stays untold
    assert!(
        !heard_soon(&mut kept_from).await,
        "followed before its project was known, an idea of a project kept from one stays untold"
    );
    // when: an idea of no project known is placed
    guarded
        .telling
        .remember_committed(
            &an_idea("idea_orphan"),
            &Place::Scene(SceneLink::from("scene_9")),
            1,
        )
        .await
        .expect("remembering should succeed");
    // then: nobody is told
    assert!(
        !heard_soon(&mut orphaned).await,
        "where an idea of no project known appears is told to nobody, as nobody can be let in to it"
    );
}

#[tokio::test]
async fn a_followed_idea_ends_once_whom_to_tell_cannot_be_known() {
    // given: a followed idea
    let guarded = guarded().await;
    let mut socket = following(&guarded.server, "idea_ada").await;

    // when: whom to let in cannot be known
    guarded.access.down.store(true, Ordering::SeqCst);
    // and: the idea is placed somewhere new
    guarded
        .telling
        .remember_committed(
            &an_idea("idea_ada"),
            &Place::Scene(SceneLink::from("scene_2")),
            2,
        )
        .await
        .expect("remembering should succeed");

    // then: the following ends as failed
    assert_eq!(
        socket.ended().await,
        "failed",
        "a change it cannot judge may be one the follower needed, so the subscription ends and its client asks again"
    );
}

#[tokio::test]
async fn whom_to_let_in_not_being_known_is_a_failure_not_a_no() {
    // given: an idea
    let guarded = guarded().await;
    // when: whom to let in cannot be known
    guarded.access.down.store(true, Ordering::SeqCst);

    // then: asking where it appears fails
    guarded
        .server
        .get("/appearances?idea=idea_ada")
        .await
        .assert_status(StatusCode::INTERNAL_SERVER_ERROR);
    // and: following it fails
    assert_eq!(
        following(&guarded.server, "idea_ada").await.ended().await,
        "failed"
    );
}
