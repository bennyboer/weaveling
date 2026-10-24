use accounts_contract::{INVITATION_DECLINED, SIGNED_UP_BY_INVITATION};
use eventsourcing::{Agent, AggregateId, EventStore, Version};
use invitations_core::{DeclineReason, InvitationEvent, InvitationId, KIND};
use invitations_messaging::DeclineInvitationListener;
use messaging::{Handler, Listener, Message, RoutingKey};
use serde_json::{Value, json};

use crate::wiring;
use crate::wiring::{ORIGIN, Wired};

const ADA: &str = "ada@example.com";

fn declined(invitation: &InvitationId) -> Message {
    told(
        json!({ "version": 0, "name": "INVITATION_DECLINED", "invitation": invitation.to_string(), "reason": "address-has-account" }),
    )
}

fn told(event: Value) -> Message {
    Message::opening(
        RoutingKey::parse(INVITATION_DECLINED).expect("a declared routing key is fine"),
        json!({
            "event": event,
            "aggregate": { "id": "account_ada", "kind": "account", "version": 4 },
            "agent": { "kind": "system", "id": null },
            "occurred_at": "2023-11-14T22:13:20Z",
        }),
        wiring::at(1_700_000_100),
    )
}

async fn invited(wired: &Wired) -> InvitationId {
    wired
        .invitations
        .issue(&wiring::an_operator(), ADA, ORIGIN)
        .await
        .expect("an operator may invite")
        .id
}

async fn happened(wired: &Wired, id: &InvitationId) -> Vec<InvitationEvent> {
    wired
        .store
        .read_from(&AggregateId::from(id), KIND, Version::of(1))
        .await
        .expect("reading should succeed")
        .into_iter()
        .map(|recorded| recorded.event)
        .collect()
}

fn listener(wired: &Wired) -> DeclineInvitationListener {
    DeclineInvitationListener::new(wired.invitations.clone())
}

#[tokio::test]
async fn an_invitation_declined_by_an_account_is_declined_once() {
    let wired = wiring::wired();
    let id = invited(&wired).await;
    let listener = listener(&wired);

    for _ in 0..2 {
        listener
            .take_in(&declined(&id))
            .await
            .expect("messages come at least once");
    }

    assert!(matches!(
        happened(&wired, &id).await.as_slice(),
        [
            InvitationEvent::Issued { .. },
            InvitationEvent::Declined {
                reason: DeclineReason::AddressHasAccount
            }
        ]
    ));
}

#[tokio::test]
async fn an_invitation_withdrawn_or_taken_up_before_it_was_declined_stays_so() {
    let wired = wiring::wired();
    let withdrawn = invited(&wired).await;
    let accepted = invited(&wired).await;
    wired
        .invitations
        .withdraw(&wiring::an_operator(), &withdrawn)
        .await
        .expect("an operator may withdraw");
    wired
        .invitations
        .accept(
            &accepted,
            wiring::at(1_700_000_050),
            &Agent::Account("account_ada".into()),
        )
        .await
        .expect("an open invitation is accepted");
    let listener = listener(&wired);

    for id in [&withdrawn, &accepted] {
        listener
            .take_in(&declined(id))
            .await
            .expect("what is settled already is no failure");
    }

    assert!(matches!(
        happened(&wired, &withdrawn).await.as_slice(),
        [_, InvitationEvent::Withdrawn]
    ));
    assert!(matches!(
        happened(&wired, &accepted).await.as_slice(),
        [_, InvitationEvent::Accepted]
    ));
}

#[tokio::test]
async fn only_a_declined_invitation_is_heard() {
    let wired = wiring::wired();
    let listener = listener(&wired);

    assert!(listener.hears(&RoutingKey::parse(INVITATION_DECLINED).expect("a declared key")));
    assert!(!listener.hears(&RoutingKey::parse(SIGNED_UP_BY_INVITATION).expect("a declared key")));
}

#[tokio::test]
async fn a_decline_naming_no_invitation_or_no_reason_is_refused() {
    let wired = wiring::wired();
    let open = invited(&wired).await;
    let listener = listener(&wired);

    for unreadable in [
        told(
            json!({ "version": 0, "name": "INVITATION_DECLINED", "invitation": "not-an-id", "reason": "address-has-account" }),
        ),
        told(
            json!({ "version": 0, "name": "INVITATION_DECLINED", "invitation": open.to_string(), "reason": "had a bad day" }),
        ),
        told(
            json!({ "version": 0, "name": "INVITATION_DECLINED", "invitation": open.to_string() }),
        ),
        told(json!({ "version": 0, "name": "SIGNED_UP" })),
        declined(&InvitationId::generate(wiring::at(1_000))),
    ] {
        assert!(
            listener.take_in(&unreadable).await.is_err(),
            "a message the listener cannot act on belongs in dead letters"
        );
    }
    assert_eq!(
        happened(&wired, &open).await.len(),
        1,
        "a reason nobody declared declines nothing"
    );
}
