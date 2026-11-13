use std::sync::Arc;

use access::{InMemoryAccess, KeptAccess};
use appearances_catalog::InMemoryAppearanceCatalog;
use appearances_core::{
    AppearancesCore, AppearancesError, IdeaLink, Place, ProjectLink, SceneLink, Subject,
};
use appearances_messaging::{RememberRolesListener, RememberTrashListener};
use eventsourcing::{AccountId, Agent};
use inbox::{InMemoryInbox, Inboxed, KeptHandler};
use members_contract::{ABANDONED, DISBANDED, FOUNDED, JOINED, LEFT, REMOVED, ROLE_CHANGED, role};
use messaging::{Listener, Message, RoutingKey};
use projects_contract::{RESTORED, TRASHED};
use serde_json::{Value, json};
use transacting::InMemoryTransactions;

use crate::committed::Committed;

const THE_LOOM: &str = "project_loom";
const AN_IDEA: &str = "idea_loom";

struct Heard {
    appearances: AppearancesCore,
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

/// Sets up the feature with accounts, and an idea of the loom placed in a scene.
/// The feature has not heard of any member yet.
async fn heard() -> Heard {
    let catalog = Arc::new(InMemoryAppearanceCatalog::new());
    catalog
        .remember_project_committed(
            &Subject::Idea(IdeaLink::from(AN_IDEA)),
            &ProjectLink::from(THE_LOOM),
        )
        .await
        .expect("remembering should succeed");
    catalog
        .remember_committed(
            &Subject::Idea(IdeaLink::from(AN_IDEA)),
            &Place::Scene(SceneLink::from("scene_1")),
            1,
        )
        .await
        .expect("remembering should succeed");
    let access = KeptAccess::new(Arc::new(InMemoryAccess::new()));
    let appearances = AppearancesCore::new(
        Arc::new(InMemoryTransactions::new()),
        catalog,
        access.clone(),
        Arc::new(access),
        Arc::new(clock::FixedClock::new(clock::at(1_700_000_000))),
    );

    Heard {
        roles: inboxed(RememberRolesListener::new(appearances.clone())),
        trash: inboxed(RememberTrashListener::new(appearances.clone())),
        appearances,
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
    async fn sees(&self, account: &str) -> bool {
        match self
            .appearances
            .places_of(AN_IDEA, &Agent::Account(AccountId::from(account)))
            .await
        {
            Ok(_) => true,
            Err(AppearancesError::IdeaNotThere) => false,
            Err(failed) => panic!("asking should succeed: {failed:?}"),
        }
    }
}

#[tokio::test]
async fn the_listeners_hear_who_is_a_member_in_which_role_and_what_is_in_the_trash() {
    // given: the listeners
    let heard = heard().await;

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
async fn where_an_idea_appears_is_told_to_the_members_of_its_project_alone() {
    // given: Ada founding the loom and Bo joining it, as the members told the appearances
    let heard = heard().await;
    hear(&heard.roles, founded("account_ada")).await;
    hear(&heard.roles, joined(2, "account_bo")).await;

    // expect: Ada and Bo see where its idea appears, and Grace does not
    assert!(heard.sees("account_ada").await);
    assert!(heard.sees("account_bo").await);
    assert!(!heard.sees("account_grace").await);
}

#[tokio::test]
async fn one_who_left_or_whose_project_was_disbanded_sees_no_more() {
    // given: Ada and Bo in the loom
    let heard = heard().await;
    hear(&heard.roles, founded("account_ada")).await;
    hear(&heard.roles, joined(2, "account_bo")).await;

    // when: Bo leaves it
    hear(&heard.roles, left(3, "account_bo")).await;

    // then: he sees no more, and Ada still does
    assert!(!heard.sees("account_bo").await);
    assert!(heard.sees("account_ada").await);
    // when: the loom is disbanded
    hear(&heard.roles, disbanded(4)).await;
    // then: Ada sees no more either
    assert!(!heard.sees("account_ada").await);
}

#[tokio::test]
async fn a_project_in_the_trash_is_seen_by_nobody_until_restored() {
    // given: Ada in the loom
    let heard = heard().await;
    hear(&heard.roles, founded("account_ada")).await;

    // when: the loom is put in the trash
    hear(&heard.trash, trashed(4)).await;

    // then: she sees no more
    assert!(!heard.sees("account_ada").await);
    // when: it is restored
    hear(&heard.trash, restored(5)).await;
    // then: she sees again
    assert!(heard.sees("account_ada").await);
}

#[tokio::test]
async fn a_joining_heard_after_the_leaving_that_followed_it_changes_nothing() {
    // given: Ada in the loom
    let heard = heard().await;
    hear(&heard.roles, founded("account_ada")).await;

    // when: Grace's leaving of the loom is heard before her joining
    hear(&heard.roles, left(4, "account_grace")).await;
    hear(&heard.roles, joined(3, "account_grace")).await;

    // then: she does not see where its idea appears
    assert!(!heard.sees("account_grace").await);
}
