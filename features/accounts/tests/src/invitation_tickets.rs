use accounts_core::{
    AccountEvent, EmailAddress, EmailAddresses, InvitationTicket, InvitationTickets,
    InvitationToken, KIND, TokenDigest,
};
use accounts_messaging::KeepInvitationTicketsListener;
use eventsourcing::{AggregateId, EventStore, Version};
use invitations_contract::{ACCEPTED, ISSUED, WITHDRAWN};
use messaging::{Handler, Listener, Message, Publisher, RoutingKey};
use serde_json::{Value, json};
use time::Duration;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

fn told(routing: &str, invitation: &str, event: Value) -> Message {
    Message::opening(
        RoutingKey::parse(routing).expect("a declared routing key is fine"),
        json!({
            "event": event,
            "aggregate": { "id": invitation, "kind": "invitation", "version": 1 },
            "agent": { "kind": "account", "id": "account_operator" },
            "occurred_at": "2023-11-14T22:13:20Z",
        }),
        wiring::at(1_700_000_000),
    )
}

fn issued(invitation: &str, to: &str, digest: &str) -> Message {
    told(
        ISSUED,
        invitation,
        json!({
            "version": 1,
            "name": "ISSUED",
            "to": to,
            "digest": digest,
            "until": "2023-11-21T22:13:20Z",
        }),
    )
}

fn withdrawn(invitation: &str) -> Message {
    told(
        WITHDRAWN,
        invitation,
        json!({ "version": 2, "name": "WITHDRAWN" }),
    )
}

async fn tell(wired: &Wired, message: Message) {
    wired
        .publisher
        .publish(message)
        .await
        .expect("telling should succeed");
    wired.settle().await;
}

async fn ticket_for(wired: &Wired, digest: &str) -> Option<InvitationTicket> {
    wired
        .secrets
        .take_ticket(&TokenDigest::kept(digest.to_owned()))
        .await
        .expect("taking should succeed")
}

#[tokio::test]
async fn an_issued_invitation_leaves_a_ticket_for_its_address_until_it_ends() {
    let wired = wiring::wired();

    tell(&wired, issued("invitation_ada", ADA, "ab12")).await;

    assert_eq!(
        ticket_for(&wired, "ab12").await,
        Some(InvitationTicket {
            invitation: "invitation_ada".to_owned(),
            to: EmailAddress::new(ADA).expect("a plain address"),
            until: wiring::at(1_700_000_000) + Duration::days(7),
        }),
        "accounts keep the ticket themselves, so signing up asks invitations nothing"
    );
}

#[tokio::test]
async fn a_withdrawn_invitation_leaves_no_ticket_and_the_others_kept() {
    let wired = wiring::wired();
    tell(&wired, issued("invitation_ada", ADA, "ab12")).await;
    tell(&wired, issued("invitation_bob", "bob@example.com", "cd34")).await;

    tell(&wired, withdrawn("invitation_ada")).await;

    assert_eq!(ticket_for(&wired, "ab12").await, None);
    assert!(ticket_for(&wired, "cd34").await.is_some());
}

#[tokio::test]
async fn an_issue_told_again_after_its_withdrawal_leaves_no_ticket() {
    let wired = wiring::wired();
    tell(&wired, issued("invitation_ada", ADA, "ab12")).await;
    tell(&wired, withdrawn("invitation_ada")).await;

    tell(&wired, issued("invitation_ada", ADA, "ab12")).await;

    assert_eq!(
        ticket_for(&wired, "ab12").await,
        None,
        "messages come at least once, so a late one must not open a withdrawn link"
    );
}

#[tokio::test]
async fn an_accepted_invitation_is_not_heard() {
    let tickets = KeepInvitationTicketsListener::new(wiring::wired().accounts.clone());

    for heard in [ISSUED, WITHDRAWN] {
        assert!(tickets.hears(&RoutingKey::parse(heard).expect("a declared key is fine")));
    }
    assert!(
        !tickets.hears(&RoutingKey::parse(ACCEPTED).expect("a declared key is fine")),
        "the ticket was taken already when its invitation is accepted"
    );
}

#[tokio::test]
async fn an_issue_to_no_address_or_until_no_time_is_refused() {
    let wired = wiring::wired();
    let tickets = KeepInvitationTicketsListener::new(wired.accounts.clone());
    let mut no_time = issued("invitation_ada", ADA, "ab12");
    no_time.payload["event"]["until"] = json!("next week");

    for unreadable in [
        issued("invitation_ada", "not an address", "ab12"),
        no_time,
        told(ISSUED, "invitation_ada", json!({ "nothing": "useful" })),
    ] {
        assert!(
            tickets.take_in(&unreadable).await.is_err(),
            "a message the listener cannot act on belongs in dead letters"
        );
    }
    assert_eq!(ticket_for(&wired, "ab12").await, None);
}

#[tokio::test]
async fn an_issue_to_an_address_with_an_account_already_is_declined_by_that_account() {
    let wired = wiring::wired();
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    let holder = wired
        .secrets
        .holder(&EmailAddress::new(ADA).expect("a plain address"))
        .await
        .expect("reading should succeed")
        .expect("the address is held");

    tell(&wired, issued("invitation_ada", ADA, "ab12")).await;
    tell(&wired, issued("invitation_ada", ADA, "ab12")).await;

    assert_eq!(
        ticket_for(&wired, "ab12").await,
        None,
        "nobody signs up twice with one address"
    );
    let declined: Vec<AccountEvent> = wired
        .store
        .read_from(&AggregateId::from(&holder), KIND, Version::of(1))
        .await
        .expect("reading should succeed")
        .into_iter()
        .map(|recorded| recorded.event)
        .filter(|event| matches!(event, AccountEvent::InvitationDeclined { .. }))
        .collect();
    assert_eq!(
        declined,
        vec![AccountEvent::InvitationDeclined {
            invitation: "invitation_ada".to_owned()
        }],
        "the operator is told once, however often the issue is told"
    );
}

#[tokio::test]
async fn an_issue_told_again_after_its_ticket_signed_the_address_up_declines_nothing() {
    let wired = wiring::wired();
    let digest = InvitationToken::presented("a token").digest();
    tell(&wired, issued("invitation_ada", ADA, digest.as_str())).await;
    wired
        .accounts
        .sign_up_by_invitation("a token", "Ada", HER_PASSWORD)
        .await
        .expect("the ticket signs the address up");

    tell(&wired, issued("invitation_ada", ADA, digest.as_str())).await;

    let holder = wired
        .secrets
        .holder(&EmailAddress::new(ADA).expect("a plain address"))
        .await
        .expect("reading should succeed")
        .expect("the address is held");
    assert!(
        !wired
            .store
            .read_from(&AggregateId::from(&holder), KIND, Version::of(1))
            .await
            .expect("reading should succeed")
            .iter()
            .any(|recorded| matches!(recorded.event, AccountEvent::InvitationDeclined { .. })),
        "an invitation taken up is not declined by the account it made"
    );
}
