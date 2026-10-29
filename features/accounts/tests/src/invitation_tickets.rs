use accounts_core::{
    AccountEvent, DeclineReason, EmailAddress, EmailAddresses, InvitationScope, InvitationTicket,
    InvitationTickets, InvitationToken, KIND, TokenDigest,
};
use accounts_messaging::KeepInvitationTicketsListener;
use eventsourcing::{AggregateId, EventStore, Version};
use invitations_contract::{ACCEPTED, DECLINED, ISSUED, RETIRED, WITHDRAWN};
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
            "scope": { "kind": "instance" },
            "by_an_operator": true,
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

fn turned_down(invitation: &str) -> Message {
    told(
        DECLINED,
        invitation,
        json!({ "version": 2, "name": "DECLINED", "reason": "turned-down" }),
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
            scope: InvitationScope::Instance,
            by_an_operator: true,
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
async fn an_invitation_turned_down_leaves_no_ticket_nor_one_told_again_and_the_others_kept() {
    let wired = wiring::wired();
    tell(&wired, issued("invitation_ada", ADA, "ab12")).await;
    tell(&wired, issued("invitation_bob", "bob@example.com", "cd34")).await;

    tell(&wired, turned_down("invitation_ada")).await;
    tell(&wired, issued("invitation_ada", ADA, "ab12")).await;

    assert_eq!(
        ticket_for(&wired, "ab12").await,
        None,
        "a link turned down signs nobody up after"
    );
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

    for heard in [ISSUED, WITHDRAWN, DECLINED, RETIRED] {
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

fn issued_into_a_project(invitation: &str, to: &str, digest: &str) -> Message {
    told(
        ISSUED,
        invitation,
        json!({
            "version": 1,
            "name": "ISSUED",
            "to": to,
            "digest": digest,
            "until": "2023-11-21T22:13:20Z",
            "scope": { "kind": "project", "project": "project_loom", "role": "author" },
            "by_an_operator": false,
        }),
    )
}

#[tokio::test]
async fn an_invitation_into_a_project_keeps_its_ticket_even_for_an_address_with_an_account() {
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

    tell(&wired, issued_into_a_project("invitation_ada", ADA, "ab12")).await;
    tell(
        &wired,
        issued_into_a_project("invitation_bo", "bo@example.com", "cd34"),
    )
    .await;

    for (digest, invitation, to) in [
        ("ab12", "invitation_ada", ADA),
        ("cd34", "invitation_bo", "bo@example.com"),
    ] {
        assert_eq!(
            ticket_for(&wired, digest).await,
            Some(InvitationTicket {
                invitation: invitation.to_owned(),
                to: EmailAddress::new(to).expect("a plain address"),
                until: wiring::at(1_700_000_000) + Duration::days(7),
                scope: InvitationScope::Project {
                    project: "project_loom".to_owned(),
                },
                by_an_operator: false,
            }),
            "an account takes a project's invitation up signed in, so its ticket waits for it"
        );
    }
    assert!(
        !wired
            .store
            .read_from(&AggregateId::from(&holder), KIND, Version::of(1))
            .await
            .expect("reading should succeed")
            .iter()
            .any(|recorded| matches!(recorded.event, AccountEvent::InvitationDeclined { .. })),
        "an account is the usual case for a project's invitation, and its owner is never told whether there is one"
    );
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
            invitation: "invitation_ada".to_owned(),
            reason: DeclineReason::AddressHasAccount,
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

fn retired(invitation: &str) -> Message {
    told(
        RETIRED,
        invitation,
        json!({ "version": 3, "name": "RETIRED" }),
    )
}

#[tokio::test]
async fn a_retired_invitation_leaves_no_ticket_nor_mark_and_the_others_stay() {
    let wired = wiring::wired();
    tell(&wired, issued("invitation_ada", ADA, "ab12")).await;
    tell(&wired, withdrawn("invitation_ada")).await;
    tell(&wired, issued("invitation_bob", "bob@example.com", "cd34")).await;

    tell(&wired, retired("invitation_ada")).await;

    assert!(
        wired
            .secrets
            .keep_ticket(
                &TokenDigest::kept("ab12".to_owned()),
                &InvitationTicket {
                    invitation: "invitation_ada".to_owned(),
                    to: EmailAddress::new(ADA).expect("a plain address"),
                    until: wiring::at(1_700_000_000),
                    scope: InvitationScope::Instance,
                    by_an_operator: true,
                },
            )
            .await
            .expect("keeping should succeed"),
        "the withdrawn invitation's mark is gone with its ticket"
    );
    assert!(ticket_for(&wired, "cd34").await.is_some());
}
