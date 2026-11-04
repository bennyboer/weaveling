use std::sync::Arc;

use async_trait::async_trait;
use authorizing::{AccessWatch, Act, Authorizer, AuthorizerError, Readable};
use axum::Extension;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, InMemoryEventStore};
use live::Broadcaster;
use live::testing::Subscriber;
use members_catalog::InMemoryMembershipCatalog;
use members_core::{MembersAuthorizer, MembersCore, ProjectLink, Role};
use members_rest::{MembersTopic, MembershipChanges};
use serde_json::json;
use time::OffsetDateTime;
use transacting::InMemoryTransactions;

const PROJECT: &str = "project_1";

struct Followed {
    core: MembersCore,
    broadcaster: Broadcaster<MembershipChanges>,
}

impl Followed {
    fn as_account(&self, account: &str) -> TestServer {
        served(&self.core, &self.broadcaster, account)
    }

    fn telling(&self, project: &str, event: serde_json::Value) {
        self.broadcaster.tell(
            MembershipChanges,
            json!({ "of": project, "version": 2, "at": "1970-01-01T00:00:00Z", "event": event })
                .to_string(),
        );
    }
}

fn served(
    core: &MembersCore,
    broadcaster: &Broadcaster<MembershipChanges>,
    account: &str,
) -> TestServer {
    TestServer::builder().http_transport().build(
        live::live_socket(live::Topics::of(vec![Arc::new(MembersTopic::new(
            core.clone(),
            broadcaster.clone(),
        ))]))
        .layer(Extension(Agent::Account(AccountId::from(account)))),
    )
}

async fn a_project_of_two() -> Followed {
    let catalog = Arc::new(InMemoryMembershipCatalog::new());
    let core = MembersCore::new(
        Arc::new(InMemoryEventStore::new()),
        Arc::new(InMemoryTransactions::new()),
        catalog.clone(),
        Arc::new(MembersAuthorizer::new(catalog)),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );
    core.found(None, PROJECT, "account_ada")
        .await
        .expect("founding should succeed");
    core.join(None, PROJECT, "account_bo", Role::Reader)
        .await
        .expect("joining should succeed");
    core.catalogue(None, &ProjectLink::from(PROJECT))
        .await
        .expect("cataloguing should succeed");

    Followed {
        core,
        broadcaster: Broadcaster::new(),
    }
}

async fn following(server: &TestServer) -> Subscriber {
    let mut socket = Subscriber::on(server).await;
    socket
        .subscribe("members", "members", &[("project", PROJECT)])
        .await;

    socket
}

#[tokio::test]
async fn a_member_following_a_project_s_members_hears_its_changes_and_no_other_project_s() {
    // given: a member following a project's members
    let followed = a_project_of_two().await;
    let mut bo_s = following(&followed.as_account("account_bo")).await;

    // when: a joining of another project is told, and then one of theirs
    followed.telling(
        "project_2",
        json!({ "name": "JOINED", "account": "account_cy", "role": "reader" }),
    );
    followed.telling(
        PROJECT,
        json!({ "name": "JOINED", "account": "account_grace", "role": "author" }),
    );

    // then: the first they hear of is the joining of theirs
    let frame = bo_s.frame().await;
    assert_eq!(frame["of"], PROJECT, "nothing of another project");
    assert_eq!(frame["event"]["account"], "account_grace");
}

#[tokio::test]
async fn a_project_s_members_are_not_there_for_whoever_may_not_read_it() {
    // given: a project of two
    let followed = a_project_of_two().await;

    // expect: a stranger's following of its members ends as not there
    assert_eq!(
        following(&followed.as_account("account_cy"))
            .await
            .ended()
            .await,
        "not-there"
    );
}

#[tokio::test]
async fn a_member_removed_hears_no_more_of_the_project_s_members() {
    // given: a member following a project's members
    let followed = a_project_of_two().await;
    let mut bo_s = following(&followed.as_account("account_bo")).await;

    // when: they are removed from it
    followed
        .core
        .remove(
            PROJECT,
            "account_bo",
            &Agent::Account(AccountId::from("account_ada")),
        )
        .await
        .expect("an owner removes a member");
    followed
        .core
        .catalogue(None, &ProjectLink::from(PROJECT))
        .await
        .expect("cataloguing should succeed");

    // then: their following ends as access lost
    assert_eq!(bo_s.ended().await, "access-lost");
}

struct Unreachable;

#[async_trait]
impl Authorizer for Unreachable {
    async fn may(
        &self,
        _agent: &Agent,
        _act: Act,
        _project: &str,
    ) -> Result<bool, AuthorizerError> {
        Err(AuthorizerError::Unreachable("the list is down".into()))
    }

    async fn readable_by(&self, _agent: &Agent) -> Result<Readable, AuthorizerError> {
        Err(AuthorizerError::Unreachable("the list is down".into()))
    }

    fn access_changed(&self, _project: &str) {}

    fn watch_access(&self, _project: &str) -> AccessWatch {
        AccessWatch::still()
    }
}

#[tokio::test]
async fn whom_to_let_follow_not_being_known_is_a_failure_not_a_no() {
    // given: who may read what cannot be learned
    let core = MembersCore::new(
        Arc::new(InMemoryEventStore::new()),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryMembershipCatalog::new()),
        Arc::new(Unreachable),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );

    // expect: following a project's members ends as failed
    assert_eq!(
        following(&served(&core, &Broadcaster::new(), "account_bo"))
            .await
            .ended()
            .await,
        "failed"
    );
}
