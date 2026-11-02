use std::sync::Arc;

use accounts_core::{
    AccountId, AccountsError, EmailAddress, EmailAddresses, InvitationScope, InvitationTicket,
    InvitationTickets, InvitationToken,
};
use accounts_messaging::RememberTrashedProjectsListener;
use inbox::{InMemoryInbox, Inboxed};
use messaging::{Listener, Message, Publisher, RoutingKey};
use projects_contract::{DELETED, RESTORED, TRASHED};
use serde_json::json;
use time::Duration;
use transacting::{InMemoryTransactions, Transactions};

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

const TOKEN: &str = "a token drawn by invitations";
const PROJECT: &str = "project_loom";

fn a_ticket(scope: InvitationScope) -> InvitationTicket {
    InvitationTicket {
        invitation: "invitation_1".to_owned(),
        to: EmailAddress::new(ADA).expect("a plain address"),
        until: wiring::at(1_700_000_000) + Duration::days(7),
        scope,
        by_an_operator: true,
    }
}

fn into_the_project() -> InvitationTicket {
    a_ticket(InvitationScope::Project {
        project: PROJECT.to_owned(),
    })
}

async fn invited(wired: &Wired, ticket: &InvitationTicket) {
    wired
        .keep_ticket(&InvitationToken::presented(TOKEN).digest(), ticket)
        .await
        .expect("keeping should succeed");
}

async fn signed_up(wired: &Wired) -> AccountId {
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");

    wired
        .secrets
        .holder(&EmailAddress::new(ADA).expect("a plain address"))
        .await
        .expect("reading should succeed")
        .expect("the address is held")
}

fn told(routing: &str, name: &str, version: u64) -> Message {
    Message::opening(
        RoutingKey::parse(routing).expect("a declared key"),
        json!({
            "event": { "version": 0, "name": name, "until": "2023-12-14T22:13:20Z" },
            "aggregate": { "id": PROJECT, "kind": "project", "version": version },
            "agent": { "kind": "account", "id": "account_owner" },
            "occurred_at": "2023-11-14T22:13:20Z",
        }),
        wiring::at(1_700_000_000),
    )
}

fn trashed(version: u64) -> Message {
    told(TRASHED, "TRASHED", version)
}

fn restored(version: u64) -> Message {
    told(RESTORED, "RESTORED", version)
}

fn deleted(version: u64) -> Message {
    told(DELETED, "DELETED", version)
}

async fn tell(wired: &Wired, message: Message) {
    wired
        .publisher
        .publish(message)
        .await
        .expect("telling should succeed");
    wired.settle().await;
}

async fn still_there(wired: &Wired) -> Option<InvitationTicket> {
    wired
        .secrets
        .find_ticket(&InvitationToken::presented(TOKEN).digest())
        .await
        .expect("finding should succeed")
}

async fn in_the_trash(wired: &Wired) -> bool {
    let transactions = InMemoryTransactions::new();
    let mut transaction = transactions
        .begin()
        .await
        .expect("beginning should succeed");

    wired
        .secrets
        .is_in_the_trash(&mut transaction, PROJECT)
        .await
        .expect("asking should succeed")
}

#[tokio::test]
async fn an_invitation_into_a_project_in_the_trash_waits_and_is_taken_up_once_it_is_restored() {
    let wired = wiring::wired();
    let ada = signed_up(&wired).await;
    invited(&wired, &into_the_project()).await;
    tell(&wired, trashed(5)).await;

    let paused = wired.accounts.take_up_invitation(&ada, TOKEN).await;
    let paused_by_id = wired
        .accounts
        .take_up_invitation_by_id(&ada, "invitation_1")
        .await;

    assert!(matches!(paused, Err(AccountsError::InvitationPaused)));
    assert!(matches!(paused_by_id, Err(AccountsError::InvitationPaused)));
    assert_eq!(
        still_there(&wired).await,
        Some(into_the_project()),
        "the ticket stays, so the invitation is good again once the project is"
    );
    tell(&wired, restored(6)).await;
    assert_eq!(
        wired
            .accounts
            .take_up_invitation(&ada, TOKEN)
            .await
            .expect("taken up once the project is restored"),
        into_the_project()
    );
}

#[tokio::test]
async fn signing_up_by_an_invitation_into_a_project_in_the_trash_waits_until_it_is_restored() {
    let wired = wiring::wired();
    invited(&wired, &into_the_project()).await;
    tell(&wired, trashed(5)).await;

    assert!(matches!(
        wired
            .accounts
            .sign_up_by_invitation(TOKEN, "Ada", HER_PASSWORD)
            .await,
        Err(AccountsError::InvitationPaused)
    ));
    assert_eq!(still_there(&wired).await, Some(into_the_project()));
    tell(&wired, restored(6)).await;
    wired
        .accounts
        .sign_up_by_invitation(TOKEN, "Ada", HER_PASSWORD)
        .await
        .expect("signed up once the project is restored");
}

#[tokio::test]
async fn an_invitation_into_the_instance_is_untouched_by_any_project_s_trash() {
    let wired = wiring::wired();
    invited(&wired, &a_ticket(InvitationScope::Instance)).await;
    tell(&wired, trashed(5)).await;

    wired
        .accounts
        .sign_up_by_invitation(TOKEN, "Ada", HER_PASSWORD)
        .await
        .expect("the instance is in no trash");
}

#[tokio::test]
async fn an_invitation_into_a_project_in_the_trash_may_still_be_turned_down() {
    let wired = wiring::wired();
    let ada = signed_up(&wired).await;
    invited(&wired, &into_the_project()).await;
    tell(&wired, trashed(5)).await;

    wired
        .accounts
        .decline_invitation(&ada, TOKEN)
        .await
        .expect("turning down needs nothing restored");

    assert_eq!(still_there(&wired).await, None);
}

#[tokio::test]
async fn an_older_trashing_heard_after_the_restoring_leaves_the_invitation_good() {
    let wired = wiring::wired();
    let ada = signed_up(&wired).await;
    invited(&wired, &into_the_project()).await;

    tell(&wired, restored(6)).await;
    tell(&wired, trashed(5)).await;

    wired
        .accounts
        .take_up_invitation(&ada, TOKEN)
        .await
        .expect("the project was restored after it was trashed");
}

#[tokio::test]
async fn a_deleted_project_s_trash_is_forgotten() {
    let wired = wiring::wired();

    tell(&wired, trashed(5)).await;
    let then = in_the_trash(&wired).await;
    tell(&wired, deleted(7)).await;

    assert!(then);
    assert!(
        !in_the_trash(&wired).await,
        "the row goes with the project, the sweep withdrawing its invitations"
    );
}

#[tokio::test]
async fn a_hearing_that_does_not_commit_leaves_the_project_out_of_the_trash() {
    let wired = wiring::wired();
    let listener = Inboxed::new(
        RememberTrashedProjectsListener::new(wired.accounts.clone()),
        Arc::new(InMemoryTransactions::refusing_commits()),
        Arc::new(InMemoryInbox::new(wired.clock.clone())),
    );
    assert!(listener.hears(&RoutingKey::parse(TRASHED).expect("a declared key")));
    assert!(listener.hears(&RoutingKey::parse(RESTORED).expect("a declared key")));
    assert!(listener.hears(&RoutingKey::parse(DELETED).expect("a declared key")));

    listener
        .handle(&trashed(5))
        .await
        .expect_err("a hearing that does not commit is not handled");

    assert!(!in_the_trash(&wired).await);
}
