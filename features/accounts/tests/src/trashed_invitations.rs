use std::sync::Arc;

use accounts_core::{
    AccountCatalog, AccountId, AccountsError, EmailAddress, EmailAddresses, InvitationScope,
    InvitationTicket, InvitationTickets, InvitationToken,
};
use accounts_messaging::RememberTrashedProjectsListener;
use eventsourcing::Agent;
use inbox::{InMemoryInbox, Inboxed};
use messaging::{Listener, Message, Publisher, RoutingKey};
use projects_contract::{DELETED, RESTORED, TRASHED};
use serde_json::json;
use time::Duration;
use transacting::InMemoryTransactions;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

const TOKEN: &str = "a token drawn by invitations";
const PROJECT: &str = "project_loom";

fn a_ticket(scope: InvitationScope) -> InvitationTicket {
    InvitationTicket {
        invitation: "invitation_1".to_owned(),
        to: EmailAddress::new(ADA).expect("a plain address"),
        until: clock::at(1_700_000_000) + Duration::days(7),
        scope,
        by_an_operator: true,
    }
}

fn a_ticket_into_the_project() -> InvitationTicket {
    a_ticket(InvitationScope::Project {
        project: PROJECT.to_owned(),
    })
}

async fn an_invitation(wired: &Wired, ticket: &InvitationTicket) {
    wired
        .keep_ticket(&InvitationToken::presented(TOKEN).digest(), ticket)
        .await
        .expect("keeping should succeed");
}

async fn an_account(wired: &Wired) -> AccountId {
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
    eventpublishing::published(
        routing,
        "project",
        PROJECT,
        version,
        json!({ "version": 0, "name": name, "until": "2023-12-14T22:13:20Z" }),
        &Agent::Account("account_owner".into()),
        clock::at(1_700_000_000),
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
    wired
        .catalog
        .is_in_the_trash(PROJECT)
        .await
        .expect("asking should succeed")
}

#[tokio::test]
async fn an_invitation_into_a_project_in_the_trash_waits_and_is_taken_up_once_it_is_restored() {
    // given: an account
    let wired = wiring::wired();
    let ada = an_account(&wired).await;
    // and: an invitation into a project to its address
    an_invitation(&wired, &a_ticket_into_the_project()).await;
    // and: the project in the trash
    tell(&wired, trashed(5)).await;

    // when: the account takes the invitation up by its link and by its id
    let paused = wired.accounts.take_up_invitation(&ada, TOKEN).await;
    let paused_by_id = wired
        .accounts
        .take_up_invitation_by_id(&ada, "invitation_1")
        .await;

    // then: both are refused as waiting
    assert!(matches!(paused, Err(AccountsError::InvitationPaused)));
    assert!(matches!(paused_by_id, Err(AccountsError::InvitationPaused)));
    // and: the ticket is kept
    assert_eq!(
        still_there(&wired).await,
        Some(a_ticket_into_the_project()),
        "the ticket stays, so the invitation is good again once the project is"
    );

    // when: the project is restored
    tell(&wired, restored(6)).await;

    // then: the invitation is taken up
    assert_eq!(
        wired
            .accounts
            .take_up_invitation(&ada, TOKEN)
            .await
            .expect("taken up once the project is restored"),
        a_ticket_into_the_project()
    );
}

#[tokio::test]
async fn signing_up_by_an_invitation_into_a_project_in_the_trash_waits_until_it_is_restored() {
    // given: an invitation into a project
    let wired = wiring::wired();
    an_invitation(&wired, &a_ticket_into_the_project()).await;
    // and: the project in the trash
    tell(&wired, trashed(5)).await;

    // then: signing up by the invitation is refused as waiting
    assert!(matches!(
        wired
            .accounts
            .sign_up_by_invitation(TOKEN, "Ada", HER_PASSWORD)
            .await,
        Err(AccountsError::InvitationPaused)
    ));
    // and: the ticket is kept
    assert_eq!(still_there(&wired).await, Some(a_ticket_into_the_project()));

    // when: the project is restored
    tell(&wired, restored(6)).await;

    // then: signing up by the invitation succeeds
    wired
        .accounts
        .sign_up_by_invitation(TOKEN, "Ada", HER_PASSWORD)
        .await
        .expect("signed up once the project is restored");
}

#[tokio::test]
async fn an_invitation_into_the_instance_is_untouched_by_any_project_s_trash() {
    // given: an invitation into the instance
    let wired = wiring::wired();
    an_invitation(&wired, &a_ticket(InvitationScope::Instance)).await;
    // and: a project in the trash
    tell(&wired, trashed(5)).await;

    // then: signing up by the invitation succeeds
    wired
        .accounts
        .sign_up_by_invitation(TOKEN, "Ada", HER_PASSWORD)
        .await
        .expect("the instance is in no trash");
}

#[tokio::test]
async fn an_invitation_into_a_project_in_the_trash_may_still_be_turned_down() {
    // given: an account
    let wired = wiring::wired();
    let ada = an_account(&wired).await;
    // and: an invitation into a project to its address
    an_invitation(&wired, &a_ticket_into_the_project()).await;
    // and: the project in the trash
    tell(&wired, trashed(5)).await;

    // when: the account turns the invitation down
    wired
        .accounts
        .decline_invitation(&ada, TOKEN)
        .await
        .expect("turning down needs nothing restored");

    // then: its ticket is gone
    assert_eq!(still_there(&wired).await, None);
}

#[tokio::test]
async fn an_older_trashing_heard_after_the_restoring_leaves_the_invitation_good() {
    // given: an account
    let wired = wiring::wired();
    let ada = an_account(&wired).await;
    // and: an invitation into a project to its address
    an_invitation(&wired, &a_ticket_into_the_project()).await;

    // when: the project's restoring is heard before its older trashing
    tell(&wired, restored(6)).await;
    tell(&wired, trashed(5)).await;

    // then: the invitation is taken up
    wired
        .accounts
        .take_up_invitation(&ada, TOKEN)
        .await
        .expect("the project was restored after it was trashed");
}

#[tokio::test]
async fn a_deleted_project_s_trash_is_forgotten() {
    // given: a project in the trash
    let wired = wiring::wired();

    tell(&wired, trashed(5)).await;
    let then = in_the_trash(&wired).await;
    // when: its deletion is heard
    tell(&wired, deleted(7)).await;

    // then: it was in the trash before
    assert!(then);
    // and: it is not now
    assert!(
        !in_the_trash(&wired).await,
        "the row goes with the project, the sweep withdrawing its invitations"
    );
}

#[tokio::test]
async fn a_hearing_that_does_not_commit_leaves_the_project_out_of_the_trash() {
    // given: a listener whose hearing does not commit
    let wired = wiring::wired();
    let listener = Inboxed::new(
        RememberTrashedProjectsListener::new(wired.accounts.clone()),
        Arc::new(InMemoryTransactions::refusing_commits()),
        Arc::new(InMemoryInbox::new(wired.clock.clone())),
    );

    // then: it hears a project trashed, restored and deleted
    assert!(listener.hears(&RoutingKey::parse(TRASHED).expect("a declared key")));
    assert!(listener.hears(&RoutingKey::parse(RESTORED).expect("a declared key")));
    assert!(listener.hears(&RoutingKey::parse(DELETED).expect("a declared key")));

    // when: a trashing is heard
    listener
        .handle(&trashed(5))
        .await
        .expect_err("a hearing that does not commit is not handled");

    // then: the project is not in the trash
    assert!(!in_the_trash(&wired).await);
}
