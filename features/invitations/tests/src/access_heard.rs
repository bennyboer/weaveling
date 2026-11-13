use acting::Acting;
use eventsourcing::Agent;
use inbox::Inboxed;
use invitations_core::{InvitationsError, Role};
use invitations_messaging::{RememberRolesListener, RememberTrashListener};
use members_contract::{ABANDONED, DISBANDED, FOUNDED, JOINED, LEFT, REMOVED, ROLE_CHANGED, role};
use messaging::{Listener, Message, RoutingKey};
use projects_contract::{RESTORED, STARTED, TRASHED};
use serde_json::{Value, json};

use crate::wiring;
use crate::wiring::{ORIGIN as INSTANCE, Wired};

const PROJECT: &str = "project_loom";
const ADA: &str = "account_ada";
const BO: &str = "account_bo";

struct Bench {
    wired: Wired,
    roles: Inboxed<RememberRolesListener>,
    trash: Inboxed<RememberTrashListener>,
}

fn bench() -> Bench {
    let wired = wiring::a_workbench_with_accounts();

    Bench {
        roles: wiring::inboxed(RememberRolesListener::new(wired.invitations.clone())),
        trash: wiring::inboxed(RememberTrashListener::new(wired.invitations.clone())),
        wired,
    }
}

impl Bench {
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

    async fn invites(&self, account: &str, address: &str) -> Result<(), InvitationsError> {
        self.wired
            .invitations
            .invite_to_project(
                &Acting {
                    agent: Agent::Account(account.into()),
                    operator: false,
                },
                PROJECT,
                Role::Reader,
                address,
                INSTANCE,
            )
            .await
            .map(|_| ())
    }
}

fn told_by_the_members(routing: &str, version: u64, event: Value) -> Message {
    eventpublishing::published(
        routing,
        "members",
        PROJECT,
        version,
        event,
        &Agent::System,
        clock::at(1_700_000_000),
    )
}

fn founded(owner: &str) -> Message {
    told_by_the_members(
        FOUNDED,
        1,
        json!({ "version": 1, "name": "FOUNDED", "owner": owner }),
    )
}

fn joined(version: u64, account: &str, as_role: &str) -> Message {
    told_by_the_members(
        JOINED,
        version,
        json!({ "version": version, "name": "JOINED", "account": account, "role": as_role }),
    )
}

fn role_changed(version: u64, account: &str, to: &str) -> Message {
    told_by_the_members(
        ROLE_CHANGED,
        version,
        json!({ "version": version, "name": "ROLE_CHANGED", "account": account, "to": to }),
    )
}

fn left(version: u64, account: &str) -> Message {
    told_by_the_members(
        LEFT,
        version,
        json!({ "version": version, "name": "LEFT", "account": account }),
    )
}

fn disbanded(version: u64) -> Message {
    told_by_the_members(
        DISBANDED,
        version,
        json!({ "version": version, "name": "DISBANDED" }),
    )
}

fn told_by_the_project(routing: &str, version: u64, event: Value) -> Message {
    eventpublishing::published(
        routing,
        "project",
        PROJECT,
        version,
        event,
        &Agent::System,
        clock::at(1_700_000_000),
    )
}

fn trashed(version: u64) -> Message {
    told_by_the_project(
        TRASHED,
        version,
        json!({ "version": version, "name": "TRASHED", "until": "2026-12-13T00:00:00Z" }),
    )
}

fn restored(version: u64) -> Message {
    told_by_the_project(
        RESTORED,
        version,
        json!({ "version": version, "name": "RESTORED" }),
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
async fn the_founder_invites_into_a_project_once_its_founding_is_heard() {
    // given: a project whose founding is not yet heard
    let bench = bench();
    assert!(matches!(
        bench.invites(ADA, "cy@example.com").await,
        Err(InvitationsError::ProjectNotThere)
    ));

    // when: its founding by ada is heard
    bench.hear(founded(ADA)).await;

    // then: ada invites into it
    assert!(bench.invites(ADA, "cy@example.com").await.is_ok());
    // and: Bo, a member of it never, finds it not there
    assert!(matches!(
        bench.invites(BO, "di@example.com").await,
        Err(InvitationsError::ProjectNotThere)
    ));
}

#[tokio::test]
async fn a_role_told_is_honored() {
    // given: a project ada founded
    let bench = bench();
    bench.hear(founded(ADA)).await;

    // when: Bo joins it as a reader
    bench.hear(joined(2, BO, role::READER)).await;

    // then: Bo is told only an owner may invite into it
    assert!(matches!(
        bench.invites(BO, "cy@example.com").await,
        Err(InvitationsError::NotAnOwner)
    ));
    // when: Bo is made an owner
    bench.hear(role_changed(3, BO, role::OWNER)).await;
    // then: Bo invites into it
    assert!(bench.invites(BO, "cy@example.com").await.is_ok());
}

#[tokio::test]
async fn one_who_left_or_whose_project_was_disbanded_invites_into_it_no_more() {
    // given: a project ada founded and Bo joined as an owner
    let bench = bench();
    bench.hear(founded(ADA)).await;
    bench.hear(joined(2, BO, role::OWNER)).await;

    // when: Bo leaves it
    bench.hear(left(3, BO)).await;

    // then: Bo finds it not there, and ada still invites into it
    assert!(matches!(
        bench.invites(BO, "cy@example.com").await,
        Err(InvitationsError::ProjectNotThere)
    ));
    assert!(bench.invites(ADA, "di@example.com").await.is_ok());
    // when: its members are disbanded
    bench.hear(disbanded(4)).await;
    // then: ada finds it not there either
    assert!(matches!(
        bench.invites(ADA, "ed@example.com").await,
        Err(InvitationsError::ProjectNotThere)
    ));
}

#[tokio::test]
async fn a_joining_heard_after_the_leaving_that_followed_it_lets_nobody_in() {
    // given: a project ada founded
    let bench = bench();
    bench.hear(founded(ADA)).await;

    // when: Bo's leaving is heard before his joining as an owner
    bench.hear(left(3, BO)).await;
    bench.hear(joined(2, BO, role::OWNER)).await;

    // then: Bo finds it not there
    assert!(matches!(
        bench.invites(BO, "cy@example.com").await,
        Err(InvitationsError::ProjectNotThere)
    ));
}

#[tokio::test]
async fn nobody_invites_into_a_project_heard_trashed_until_heard_restored() {
    // given: a project ada founded
    let bench = bench();
    bench.hear(founded(ADA)).await;

    // when: it is heard trashed
    bench.hear_of_the_trash(trashed(2)).await;

    // then: ada finds it not there
    assert!(matches!(
        bench.invites(ADA, "cy@example.com").await,
        Err(InvitationsError::ProjectNotThere)
    ));
    // when: it is heard restored
    bench.hear_of_the_trash(restored(3)).await;
    // then: ada invites into it again
    assert!(bench.invites(ADA, "cy@example.com").await.is_ok());
}
