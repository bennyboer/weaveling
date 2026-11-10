use std::sync::Arc;

use accounts_core::{AccountId, AccountsError, EmailAddress, EmailAddresses};
use accounts_messaging::RememberProjectMembersListener;
use eventsourcing::Agent;
use inbox::{InMemoryInbox, Inboxed};
use members_contract::{DISBANDED, FOUNDED, JOINED, ROLE_CHANGED, role};
use messaging::{Listener, Message, RoutingKey};
use serde_json::{Value, json};
use transacting::InMemoryTransactions;

use crate::wiring;
use crate::wiring::{HER_PASSWORD, Wired};

const ADA: &str = "ada@example.com";
const GRACE: &str = "grace@example.com";
const PROJECT: &str = "project_shared";

async fn a_verified_account(wired: &Wired, email: &str) -> AccountId {
    wired
        .accounts
        .sign_up(email, "Someone", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    wired
        .accounts
        .verify(email, &wired.last_code_sent_to(email))
        .await
        .expect("the code sent should verify");

    wired
        .secrets
        .holder(&EmailAddress::new(email).expect("a plain address"))
        .await
        .expect("reading should succeed")
        .expect("the address should be held")
}

fn heard_by(wired: &Wired) -> Inboxed<RememberProjectMembersListener> {
    Inboxed::new(
        RememberProjectMembersListener::new(wired.accounts.clone()),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryInbox::new(wired.clock.clone())),
    )
}

fn told(routing: &str, version: u64, event: Value) -> Message {
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

async fn hear(listener: &Inboxed<RememberProjectMembersListener>, message: Message) {
    listener
        .handle(&message)
        .await
        .expect("a change to the members is taken in");
}

/// Ada founds a project, and grace joins it as an author.
async fn ada_alone_owning_a_project_grace_is_in(
    wired: &Wired,
) -> (
    AccountId,
    AccountId,
    Inboxed<RememberProjectMembersListener>,
) {
    let ada = a_verified_account(wired, ADA).await;
    let grace = a_verified_account(wired, GRACE).await;
    let listener = heard_by(wired);
    hear(
        &listener,
        told(
            FOUNDED,
            1,
            json!({ "version": 1, "name": "FOUNDED", "owner": ada.to_string() }),
        ),
    )
    .await;
    hear(
        &listener,
        told(
            JOINED,
            2,
            json!({ "version": 2, "name": "JOINED", "account": grace.to_string(), "role": role::AUTHOR }),
        ),
    )
    .await;

    (ada, grace, listener)
}

#[tokio::test]
async fn the_last_owner_of_a_project_others_are_in_hands_it_over_before_the_account_is_deleted() {
    // given: ada alone owns a project grace is in, as the members told accounts
    let wired = wiring::wired();
    let (ada, grace, listener) = ada_alone_owning_a_project_grace_is_in(&wired).await;
    // expect: the listener hears every change to members
    for routing in [FOUNDED, JOINED, ROLE_CHANGED, DISBANDED] {
        assert!(listener.hears(&RoutingKey::parse(routing).expect("a declared key")));
    }

    // when: ada asks for her account to be deleted
    let refused = wired.accounts.ask_deletion(&ada, HER_PASSWORD).await;

    // then: it is refused, naming the project
    assert!(matches!(
        refused,
        Err(AccountsError::ProjectsToHandOver(projects)) if projects == [PROJECT]
    ));
    // when: grace is made an owner too
    hear(
        &listener,
        told(
            ROLE_CHANGED,
            3,
            json!({ "version": 3, "name": "ROLE_CHANGED", "account": grace.to_string(), "to": role::OWNER }),
        ),
    )
    .await;
    // then: ada may ask
    wired
        .accounts
        .ask_deletion(&ada, HER_PASSWORD)
        .await
        .expect("with another owner, nothing is left to hand over");
}

#[tokio::test]
async fn a_project_in_the_trash_or_deleted_is_nothing_to_hand_over() {
    for gone in ["trashed", "disbanded"] {
        // given: ada alone owns a project grace is in
        let wired = wiring::wired();
        let (ada, _, listener) = ada_alone_owning_a_project_grace_is_in(&wired).await;

        // when: the project is put in the trash, or deleted and its members disbanded
        match gone {
            "trashed" => wired
                .accounts
                .remember_trash(None, PROJECT, 4, true)
                .await
                .expect("remembering should succeed"),
            _ => {
                hear(
                    &listener,
                    told(DISBANDED, 3, json!({ "version": 3, "name": "DISBANDED" })),
                )
                .await
            }
        }

        // then: ada may ask for her account to be deleted
        wired
            .accounts
            .ask_deletion(&ada, HER_PASSWORD)
            .await
            .unwrap_or_else(|refused| panic!("a project {gone} keeps nothing back: {refused}"));
    }
}

#[tokio::test]
async fn the_operator_is_refused_too_while_there_is_a_project_to_hand_over() {
    // given: ada alone owns a project grace is in, and an operator
    let wired = wiring::wired();
    let (ada, _, _) = ada_alone_owning_a_project_grace_is_in(&wired).await;
    let operator = a_verified_account(&wired, "hedy@example.com").await;
    let code = wired
        .accounts
        .draw_operator_code()
        .await
        .expect("drawing should succeed")
        .expect("with nobody appointed a code is drawn");
    wired
        .accounts
        .become_operator(&operator, code.as_str())
        .await
        .expect("the code drawn makes an operator");

    // expect: the operator may not mark her account either
    assert!(matches!(
        wired
            .accounts
            .mark_for_deletion(&operator, &ada, None)
            .await,
        Err(AccountsError::ProjectsToHandOver(_))
    ));
}
