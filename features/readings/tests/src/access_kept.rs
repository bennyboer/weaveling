use std::sync::Arc;

use crate::sample::SampleBook;
use access::{InMemoryAccess, KeptAccess};
use eventsourcing::{AccountId, Agent, InMemoryEventStore};
use inbox::{InMemoryInbox, Inboxed, KeptHandler};
use members_contract::{ABANDONED, DISBANDED, FOUNDED, JOINED, LEFT, REMOVED, ROLE_CHANGED, role};
use messaging::{Listener, Message, RoutingKey};
use projects_contract::{RESTORED, TRASHED};
use readings_catalog::{
    InMemoryAccountNameCatalog, InMemoryReadingCopyCatalog, InMemoryReadingCopyContents,
};
use readings_core::{NobodyFollows, ReadingsCore, ReadingsError};
use readings_messaging::{RememberRolesListener, RememberTrashListener};
use serde_json::{Value, json};
use transacting::InMemoryTransactions;

const THE_LOOM: &str = "project_loom";

struct Heard {
    readings: ReadingsCore,
    roles: Inboxed<RememberRolesListener>,
    trash: Inboxed<RememberTrashListener>,
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

/// The feature with accounts, where no member has been heard of yet.
fn heard() -> Heard {
    let access = KeptAccess::new(Arc::new(InMemoryAccess::new()));
    let readings = ReadingsCore::new(
        Arc::new(SampleBook),
        Arc::new(SampleBook),
        Arc::new(InMemoryReadingCopyCatalog::new()),
        Arc::new(InMemoryAccountNameCatalog::new()),
        Arc::new(InMemoryReadingCopyContents::new()),
        Arc::new(InMemoryEventStore::new()),
        Arc::new(NobodyFollows),
        Arc::new(InMemoryTransactions::new()),
        access.clone(),
        Arc::new(access),
        Arc::new(clock::FixedClock::new(clock::at(1_700_000_000))),
    );

    Heard {
        roles: inboxed(RememberRolesListener::new(readings.clone())),
        trash: inboxed(RememberTrashListener::new(readings.clone())),
        readings,
    }
}

fn told(routing: &str, kind: &str, version: u64, event: Value) -> Message {
    eventpublishing::published(
        routing,
        kind,
        THE_LOOM,
        version,
        event,
        &Agent::System,
        clock::at(1_700_000_000),
    )
}

fn founded(owner: &str) -> Message {
    told(
        FOUNDED,
        "members",
        1,
        json!({ "version": 1, "name": "FOUNDED", "owner": owner }),
    )
}

fn joined(version: u64, account: &str) -> Message {
    told(
        JOINED,
        "members",
        version,
        json!({ "version": version, "name": "JOINED", "account": account, "role": role::READER }),
    )
}

fn left(version: u64, account: &str) -> Message {
    told(
        LEFT,
        "members",
        version,
        json!({ "version": version, "name": "LEFT", "account": account }),
    )
}

fn disbanded(version: u64) -> Message {
    told(
        DISBANDED,
        "members",
        version,
        json!({ "version": version, "name": "DISBANDED" }),
    )
}

fn trashed(version: u64) -> Message {
    told(
        TRASHED,
        "project",
        version,
        json!({ "version": version, "name": "TRASHED", "until": "2026-12-13T00:00:00Z" }),
    )
}

fn restored(version: u64) -> Message {
    told(
        RESTORED,
        "project",
        version,
        json!({ "version": version, "name": "RESTORED" }),
    )
}

async fn hear<H: KeptHandler>(listener: &Inboxed<H>, message: Message) {
    listener
        .handle(&message)
        .await
        .expect("who may do what is taken in");
}

impl Heard {
    async fn reads(&self, account: &str) -> bool {
        match self
            .readings
            .live_book(THE_LOOM, &Agent::Account(AccountId::from(account)))
            .await
        {
            Ok(_) => true,
            Err(ReadingsError::ProjectNotThere) => false,
            Err(failed) => panic!("asking should succeed: {failed:?}"),
        }
    }
}

#[tokio::test]
async fn the_listeners_hear_who_is_a_member_in_which_role_and_what_is_in_the_trash() {
    // given: the listeners
    let heard = heard();

    // expect: one hears every word on who is a member in which role
    for routing in [FOUNDED, JOINED, ROLE_CHANGED, LEFT, REMOVED, DISBANDED] {
        assert!(
            heard
                .roles
                .hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
    // and: not the abandoning that follows the last removal
    assert!(
        !heard
            .roles
            .hears(&RoutingKey::parse(ABANDONED).expect("a declared key"))
    );
    // and: the other a project put in the trash and restored
    for routing in [TRASHED, RESTORED] {
        assert!(
            heard
                .trash
                .hears(&RoutingKey::parse(routing).expect("a declared key")),
            "{routing}"
        );
    }
}

#[tokio::test]
async fn a_project_s_book_is_read_by_its_members_alone() {
    // given: Ada founding the loom and Bo joining it, as the members told the readings
    let heard = heard();
    hear(&heard.roles, founded("account_ada")).await;
    hear(&heard.roles, joined(2, "account_bo")).await;

    // expect: Ada and Bo read its book, and Grace does not
    assert!(heard.reads("account_ada").await);
    assert!(heard.reads("account_bo").await);
    assert!(!heard.reads("account_grace").await);
}

#[tokio::test]
async fn one_who_left_or_whose_project_was_disbanded_reads_no_more() {
    // given: Ada and Bo in the loom
    let heard = heard();
    hear(&heard.roles, founded("account_ada")).await;
    hear(&heard.roles, joined(2, "account_bo")).await;

    // when: Bo leaves it
    hear(&heard.roles, left(3, "account_bo")).await;

    // then: he reads no more, and Ada still does
    assert!(!heard.reads("account_bo").await);
    assert!(heard.reads("account_ada").await);
    // when: the loom is disbanded
    hear(&heard.roles, disbanded(4)).await;
    // then: Ada reads no more either
    assert!(!heard.reads("account_ada").await);
}

#[tokio::test]
async fn a_project_in_the_trash_is_read_by_nobody_until_restored() {
    // given: Ada in the loom
    let heard = heard();
    hear(&heard.roles, founded("account_ada")).await;

    // when: the loom is put in the trash
    hear(&heard.trash, trashed(4)).await;

    // then: she reads no more
    assert!(!heard.reads("account_ada").await);
    // when: it is restored
    hear(&heard.trash, restored(5)).await;
    // then: she reads again
    assert!(heard.reads("account_ada").await);
}

#[tokio::test]
async fn a_joining_heard_after_the_leaving_that_followed_it_changes_nothing() {
    // given: Ada in the loom
    let heard = heard();
    hear(&heard.roles, founded("account_ada")).await;

    // when: Grace's leaving of the loom is heard before her joining
    hear(&heard.roles, left(4, "account_grace")).await;
    hear(&heard.roles, joined(3, "account_grace")).await;

    // then: she does not read its book
    assert!(!heard.reads("account_grace").await);
}
