use std::sync::Arc;

use boards_core::{BoardsCore, BoardsError, IdeaLink, Size, Spot};
use boards_messaging::{RememberRolesListener, RememberTrashListener};
use eventsourcing::{AccountId, Agent, ServiceError};
use inbox::{InMemoryInbox, Inboxed, KeptHandler};
use members_contract::{ABANDONED, DISBANDED, FOUNDED, JOINED, LEFT, REMOVED, ROLE_CHANGED, role};
use messaging::{Listener, Message, RoutingKey};
use projects_contract::{RESTORED, TRASHED};
use serde_json::{Value, json};
use transacting::InMemoryTransactions;

use crate::wiring;

const THE_LOOM: &str = "project_loom";

fn ada() -> Agent {
    Agent::Account(AccountId::from("account_ada"))
}

fn bo() -> Agent {
    Agent::Account(AccountId::from("account_bo"))
}

fn grace() -> Agent {
    Agent::Account(AccountId::from("account_grace"))
}

fn boards() -> BoardsCore {
    wiring::wired_with_accounts(Arc::new(clock::FixedClock::new(clock::at(1_700_000_000)))).boards
}

fn inboxed<H: KeptHandler>(handler: H) -> Inboxed<H> {
    Inboxed::new(
        handler,
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryInbox::new(Arc::new(clock::FixedClock::new(
            clock::at(1_700_000_000),
        )))),
    )
}

fn roles_heard_by(boards: &BoardsCore) -> Inboxed<RememberRolesListener> {
    inboxed(RememberRolesListener::new(boards.clone()))
}

fn trash_heard_by(boards: &BoardsCore) -> Inboxed<RememberTrashListener> {
    inboxed(RememberTrashListener::new(boards.clone()))
}

fn told(routing: &str, version: u64, event: Value) -> Message {
    eventpublishing::published(
        routing,
        "members",
        THE_LOOM,
        version,
        event,
        &Agent::System,
        clock::at(1_700_000_000),
    )
}

fn founded() -> Message {
    told(
        FOUNDED,
        1,
        json!({ "version": 1, "name": "FOUNDED", "owner": "account_ada" }),
    )
}

fn joined(version: u64, account: &str, as_role: &str) -> Message {
    told(
        JOINED,
        version,
        json!({ "version": version, "name": "JOINED", "account": account, "role": as_role }),
    )
}

fn left(version: u64, account: &str) -> Message {
    told(
        LEFT,
        version,
        json!({ "version": version, "name": "LEFT", "account": account }),
    )
}

fn the_project(routing: &str, version: u64, event: Value) -> Message {
    eventpublishing::published(
        routing,
        "project",
        THE_LOOM,
        version,
        event,
        &Agent::System,
        clock::at(1_700_000_000),
    )
}

async fn hear<H: Listener>(listener: &H, message: Message) {
    listener
        .handle(&message)
        .await
        .expect("a word on who may do what is taken in");
}

/// Ada founds the loom, Bo joins it as an author and Grace as a reader.
async fn the_members_heard(listener: &Inboxed<RememberRolesListener>) {
    hear(listener, founded()).await;
    hear(listener, joined(2, "account_bo", role::AUTHOR)).await;
    hear(listener, joined(3, "account_grace", role::READER)).await;
}

async fn reads(boards: &BoardsCore, agent: &Agent) -> bool {
    match boards.open(THE_LOOM, agent).await {
        Ok(_) => true,
        Err(BoardsError::ProjectNotThere) => false,
        Err(failure) => panic!("opening failed: {failure}"),
    }
}

async fn pins(boards: &BoardsCore, idea: &str, agent: &Agent) -> Result<(), BoardsError> {
    let board = boards.open(THE_LOOM, &Agent::System).await?.id;

    boards
        .pin(
            &board.to_string(),
            IdeaLink::from(idea),
            Spot::ORIGIN,
            Size::CARD,
            None,
            agent,
        )
        .await
        .map(|_| ())
}

#[tokio::test]
async fn the_listeners_hear_who_joins_goes_and_changes_role_and_the_trash_but_not_abandoning() {
    // given: the listeners
    let boards = boards();
    let roles = roles_heard_by(&boards);
    let trash = trash_heard_by(&boards);

    // expect: one hears every word on who is a member
    for routing in [FOUNDED, JOINED, ROLE_CHANGED, LEFT, REMOVED, DISBANDED] {
        assert!(
            roles.hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
    // and: not the abandoning that follows the last removal
    assert!(!roles.hears(&RoutingKey::parse(ABANDONED).expect("a declared key")));
    // and: the other a project put in the trash and restored
    for routing in [TRASHED, RESTORED] {
        assert!(
            trash.hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
}

#[tokio::test]
async fn each_member_may_do_with_the_board_what_their_role_told_permits() {
    // given: Ada owning the loom, Bo writing in it and Grace reading it, as the members told the boards
    let boards = boards();
    the_members_heard(&roles_heard_by(&boards)).await;

    // expect: Ada and Bo pin ideas to its board
    pins(&boards, "idea_ada", &ada())
        .await
        .expect("an owner pins ideas");
    pins(&boards, "idea_bo", &bo())
        .await
        .expect("an author pins ideas");
    // and: Grace opens it but is told only an author may pin one
    assert!(reads(&boards, &grace()).await);
    assert!(matches!(
        pins(&boards, "idea_grace", &grace()).await,
        Err(BoardsError::NotAnAuthor)
    ));
    // and: to one told of in no role the project is not there
    assert!(!reads(&boards, &Agent::Account(AccountId::from("account_eve"))).await);
}

#[tokio::test]
async fn a_role_changed_is_honored() {
    // given: Grace reading the loom
    let boards = boards();
    let listener = roles_heard_by(&boards);
    the_members_heard(&listener).await;

    // when: she is made an author
    hear(
        &listener,
        told(
            ROLE_CHANGED,
            4,
            json!({ "version": 4, "name": "ROLE_CHANGED", "account": "account_grace", "to": role::AUTHOR }),
        ),
    )
    .await;

    // then: she pins ideas to its board
    pins(&boards, "idea_grace", &grace())
        .await
        .expect("an author pins ideas");
}

#[tokio::test]
async fn one_who_left_or_was_removed_or_whose_project_was_disbanded_opens_its_board_no_more() {
    // given: Ada, Bo and Grace in the loom
    let boards = boards();
    let listener = roles_heard_by(&boards);
    the_members_heard(&listener).await;

    // when: Grace leaves it and Bo is removed
    hear(&listener, left(4, "account_grace")).await;
    hear(
        &listener,
        told(
            REMOVED,
            5,
            json!({ "version": 5, "name": "REMOVED", "account": "account_bo" }),
        ),
    )
    .await;

    // then: neither opens its board, and Ada still does
    assert!(!reads(&boards, &grace()).await);
    assert!(!reads(&boards, &bo()).await);
    assert!(reads(&boards, &ada()).await);
    // when: the loom is disbanded
    hear(
        &listener,
        told(DISBANDED, 6, json!({ "version": 6, "name": "DISBANDED" })),
    )
    .await;

    // then: Ada opens it no more either
    assert!(!reads(&boards, &ada()).await);
}

#[tokio::test]
async fn a_joining_heard_after_the_leaving_that_followed_it_lets_nobody_in() {
    // given: Ada in the loom
    let boards = boards();
    let listener = roles_heard_by(&boards);
    hear(&listener, founded()).await;

    // when: Grace's leaving of the loom is heard before her joining
    hear(&listener, left(4, "account_grace")).await;
    hear(&listener, joined(3, "account_grace", role::READER)).await;

    // then: she does not open its board
    assert!(!reads(&boards, &grace()).await);
}

#[tokio::test]
async fn a_project_in_the_trash_lets_nobody_at_its_board_until_restored() {
    // given: Ada, Bo and Grace in the loom
    let boards = boards();
    the_members_heard(&roles_heard_by(&boards)).await;
    let trash = trash_heard_by(&boards);

    // when: the loom is put in the trash
    hear(
        &trash,
        the_project(
            TRASHED,
            4,
            json!({ "version": 4, "name": "TRASHED", "until": "2026-12-13T00:00:00Z" }),
        ),
    )
    .await;

    // then: not even Ada opens its board or pins to it
    assert!(!reads(&boards, &ada()).await);
    assert!(matches!(
        pins(&boards, "idea_ada", &ada()).await,
        Err(BoardsError::Events(ServiceError::NotFound { .. }))
    ));
    // when: it is restored
    hear(
        &trash,
        the_project(RESTORED, 5, json!({ "version": 5, "name": "RESTORED" })),
    )
    .await;

    // then: Ada pins ideas to its board again
    pins(&boards, "idea_ada", &ada())
        .await
        .expect("an owner pins ideas in a project restored");
}
