use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{AccountId, Agent, ServiceError};
use inbox::{InMemoryInbox, Inboxed, KeptHandler};
use members_contract::{ABANDONED, DISBANDED, FOUNDED, JOINED, LEFT, REMOVED, ROLE_CHANGED, role};
use messaging::{Listener, Message, RoutingKey};
use projects_contract::{RESTORED, STARTED, TRASHED};
use projects_core::{ProjectError, ProjectsError, RawDetails};
use projects_messaging::{RememberRolesListener, RememberTrashListener};
use serde_json::{Value, json};
use transacting::InMemoryTransactions;

use crate::wiring;
use crate::wiring::Wired;

const ADA: &str = "account_ada";
const BO: &str = "account_bo";

fn an_account(id: &str) -> Agent {
    Agent::Account(AccountId::from(id))
}

fn inboxed<H: KeptHandler>(handler: H) -> Inboxed<H> {
    Inboxed::new(
        handler,
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(clock::at(
            1_700_000_000,
        ))))),
    )
}

struct Bench {
    wired: Wired,
    roles: Inboxed<RememberRolesListener>,
    trash: Inboxed<RememberTrashListener>,
}

fn bench() -> Bench {
    let wired = wiring::wired_with_accounts(Arc::new(FixedClock::new(clock::at(1_000))));

    Bench {
        roles: inboxed(RememberRolesListener::new(wired.projects.clone())),
        trash: inboxed(RememberTrashListener::new(wired.projects.clone())),
        wired,
    }
}

impl Bench {
    async fn a_project(&self) -> String {
        self.wired
            .projects
            .start("The Loom", &RawDetails::default(), &an_account(ADA))
            .await
            .expect("anyone signed in starts a project")
            .0
            .to_string()
    }

    async fn hear(&self, message: Message) {
        self.roles
            .handle(&message)
            .await
            .expect("a change to the members is taken in");
    }

    async fn hear_of_the_trash(&self, message: Message) {
        self.trash
            .handle(&message)
            .await
            .expect("a project trashed or restored is taken in");
    }

    async fn reads(&self, project: &str, account: &str) -> Result<(), ProjectsError> {
        self.wired
            .projects
            .read(project, &an_account(account))
            .await
            .map(|_| ())
    }

    async fn renames(&self, project: &str, account: &str) -> Result<(), ProjectsError> {
        self.wired
            .projects
            .rename(project, "The Heddle", None, &an_account(account))
            .await
            .map(|_| ())
    }
}

fn told_by_the_members(routing: &str, project: &str, version: u64, event: Value) -> Message {
    eventpublishing::published(
        routing,
        "members",
        project,
        version,
        event,
        &Agent::System,
        clock::at(1_700_000_000),
    )
}

fn founded(project: &str, owner: &str) -> Message {
    told_by_the_members(
        FOUNDED,
        project,
        1,
        json!({ "version": 1, "name": "FOUNDED", "owner": owner }),
    )
}

fn joined(project: &str, version: u64, account: &str, as_role: &str) -> Message {
    told_by_the_members(
        JOINED,
        project,
        version,
        json!({ "version": version, "name": "JOINED", "account": account, "role": as_role }),
    )
}

fn role_changed(project: &str, version: u64, account: &str, to: &str) -> Message {
    told_by_the_members(
        ROLE_CHANGED,
        project,
        version,
        json!({ "version": version, "name": "ROLE_CHANGED", "account": account, "to": to }),
    )
}

fn left(project: &str, version: u64, account: &str) -> Message {
    told_by_the_members(
        LEFT,
        project,
        version,
        json!({ "version": version, "name": "LEFT", "account": account }),
    )
}

fn disbanded(project: &str, version: u64) -> Message {
    told_by_the_members(
        DISBANDED,
        project,
        version,
        json!({ "version": version, "name": "DISBANDED" }),
    )
}

fn told_by_the_project(routing: &str, project: &str, version: u64, event: Value) -> Message {
    eventpublishing::published(
        routing,
        "project",
        project,
        version,
        event,
        &Agent::System,
        clock::at(1_700_000_000),
    )
}

fn trashed(project: &str, version: u64) -> Message {
    told_by_the_project(
        TRASHED,
        project,
        version,
        json!({ "version": version, "name": "TRASHED", "until": "2026-12-13T00:00:00Z" }),
    )
}

fn restored(project: &str, version: u64) -> Message {
    told_by_the_project(
        RESTORED,
        project,
        version,
        json!({ "version": version, "name": "RESTORED" }),
    )
}

fn is_not_found(result: &Result<(), ProjectsError>) -> bool {
    matches!(
        result,
        Err(ProjectsError::Events(ServiceError::NotFound { .. }))
    )
}

fn key(routing: &str) -> RoutingKey {
    RoutingKey::parse(routing).expect("a declared key")
}

#[tokio::test]
async fn the_listeners_hear_who_is_a_member_and_what_is_in_the_trash_but_not_abandoning() {
    // given: the listeners
    let bench = bench();

    // expect: the one on roles hears every word on who is a member
    for routing in [FOUNDED, JOINED, ROLE_CHANGED, LEFT, REMOVED, DISBANDED] {
        assert!(bench.roles.hears(&key(routing)), "{routing}");
    }
    // and: not the abandoning that follows the last removal
    assert!(!bench.roles.hears(&key(ABANDONED)));
    // and: the one on the trash hears a project trashed and restored, and no other change of it
    assert!(bench.trash.hears(&key(TRASHED)));
    assert!(bench.trash.hears(&key(RESTORED)));
    assert!(!bench.trash.hears(&key(STARTED)));
}

#[tokio::test]
async fn the_founder_uses_a_project_once_its_founding_is_heard() {
    // given: a project ada started
    let bench = bench();
    let project = bench.a_project().await;
    // and: not yet heard founded
    assert!(is_not_found(&bench.reads(&project, ADA).await));

    // when: its founding is heard
    bench.hear(founded(&project, ADA)).await;

    // then: ada reads and renames it
    assert!(bench.reads(&project, ADA).await.is_ok());
    assert!(bench.renames(&project, ADA).await.is_ok());
    // and: Bo, a member of it never, finds nothing
    assert!(is_not_found(&bench.reads(&project, BO).await));
}

#[tokio::test]
async fn a_role_told_is_honored() {
    // given: a project ada founded
    let bench = bench();
    let project = bench.a_project().await;
    bench.hear(founded(&project, ADA)).await;

    // when: Bo joins it as a reader
    bench.hear(joined(&project, 2, BO, role::READER)).await;

    // then: Bo reads it, and is told only an owner may rename it
    assert!(bench.reads(&project, BO).await.is_ok());
    assert!(matches!(
        bench.renames(&project, BO).await,
        Err(ProjectsError::NotAnOwner)
    ));
    // when: Bo is made an owner
    bench.hear(role_changed(&project, 3, BO, role::OWNER)).await;
    // then: Bo renames it
    assert!(bench.renames(&project, BO).await.is_ok());
}

#[tokio::test]
async fn one_who_left_or_whose_project_was_disbanded_may_do_nothing_in_it_any_more() {
    // given: a project ada founded and Bo joined
    let bench = bench();
    let project = bench.a_project().await;
    bench.hear(founded(&project, ADA)).await;
    bench.hear(joined(&project, 2, BO, role::AUTHOR)).await;

    // when: Bo leaves it
    bench.hear(left(&project, 3, BO)).await;

    // then: Bo finds nothing, and ada still reads it
    assert!(is_not_found(&bench.reads(&project, BO).await));
    assert!(bench.reads(&project, ADA).await.is_ok());
    // when: its members are disbanded
    bench.hear(disbanded(&project, 4)).await;
    // then: ada finds nothing either
    assert!(is_not_found(&bench.reads(&project, ADA).await));
}

#[tokio::test]
async fn a_joining_heard_after_the_leaving_that_followed_it_lets_nobody_in() {
    // given: a project ada founded
    let bench = bench();
    let project = bench.a_project().await;
    bench.hear(founded(&project, ADA)).await;

    // when: Bo's leaving is heard before his joining
    bench.hear(left(&project, 3, BO)).await;
    bench.hear(joined(&project, 2, BO, role::OWNER)).await;

    // then: Bo finds nothing
    assert!(is_not_found(&bench.reads(&project, BO).await));
}

#[tokio::test]
async fn a_project_heard_trashed_may_only_be_restored_until_heard_restored() {
    // given: a project ada founded
    let bench = bench();
    let project = bench.a_project().await;
    bench.hear(founded(&project, ADA)).await;

    // when: it is heard trashed
    bench.hear_of_the_trash(trashed(&project, 2)).await;

    // then: ada finds nothing to read
    assert!(is_not_found(&bench.reads(&project, ADA).await));
    // and: her renaming is refused as in the trash, since she may restore it
    assert!(matches!(
        bench.renames(&project, ADA).await,
        Err(ProjectsError::Events(ServiceError::Refused(
            ProjectError::Trashed
        )))
    ));
    // when: it is heard restored
    bench.hear_of_the_trash(restored(&project, 3)).await;
    // then: ada reads it again
    assert!(bench.reads(&project, ADA).await.is_ok());
}
