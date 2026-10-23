use accounts_contract::{SIGNED_UP, SIGNED_UP_BY_INVITATION};
use eventsourcing::{AggregateId, EventStore, Version};
use invitations_core::{InvitationEvent, InvitationId, KIND};
use invitations_messaging::AcceptTakenUpInvitationListener;
use messaging::{Handler, Listener, Message, RoutingKey};
use serde_json::{Value, json};
use time::{Duration, OffsetDateTime};

use crate::wiring;
use crate::wiring::{ORIGIN, Wired, an_operator};

const ADA: &str = "ada@example.com";

fn signed_up(invitation: &InvitationId, at: OffsetDateTime) -> Message {
    told(
        json!({ "version": 0, "name": "SIGNED_UP_BY_INVITATION", "invitation": invitation.to_string() }),
        at,
    )
}

fn told(event: Value, at: OffsetDateTime) -> Message {
    Message::opening(
        RoutingKey::parse(SIGNED_UP_BY_INVITATION).expect("a declared routing key is fine"),
        json!({
            "event": event,
            "aggregate": { "id": "account_ada", "kind": "account", "version": 1 },
            "agent": { "kind": "account", "id": "account_ada" },
            "occurred_at": "2023-11-14T22:13:20Z",
        }),
        at,
    )
}

async fn invited(wired: &Wired) -> InvitationId {
    wired
        .invitations
        .issue(&an_operator(), ADA, ORIGIN)
        .await
        .expect("an operator may invite")
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

fn listener(wired: &Wired) -> AcceptTakenUpInvitationListener {
    AcceptTakenUpInvitationListener::new(wired.invitations.clone())
}

#[tokio::test]
async fn an_invitation_an_account_was_made_by_is_accepted() {
    let wired = wiring::wired();
    let id = invited(&wired).await;

    listener(&wired)
        .take_in(&signed_up(&id, wiring::at(1_700_000_100)))
        .await
        .expect("an open invitation is accepted");

    assert!(matches!(
        happened(&wired, &id).await.as_slice(),
        [InvitationEvent::Issued { .. }, InvitationEvent::Accepted]
    ));
}

#[tokio::test]
async fn an_invitation_is_accepted_as_of_the_sign_up_however_late_it_is_heard() {
    let wired = wiring::wired();
    let id = invited(&wired).await;
    wired
        .clock
        .set(wiring::at(1_700_000_000) + Duration::days(30));

    listener(&wired)
        .take_in(&signed_up(
            &id,
            wiring::at(1_700_000_000) + Duration::days(7),
        ))
        .await
        .expect("the account was made while the invitation held");

    assert!(matches!(
        happened(&wired, &id).await.as_slice(),
        [_, InvitationEvent::Accepted]
    ));
}

#[tokio::test]
async fn a_sign_up_told_again_or_after_a_withdrawal_changes_nothing() {
    let wired = wiring::wired();
    let accepted = invited(&wired).await;
    let withdrawn = invited(&wired).await;
    wired
        .invitations
        .withdraw(&an_operator(), &withdrawn)
        .await
        .expect("an operator may withdraw");
    let listener = listener(&wired);
    listener
        .take_in(&signed_up(&accepted, wiring::at(1_700_000_100)))
        .await
        .expect("an open invitation is accepted");

    for id in [&accepted, &withdrawn] {
        listener
            .take_in(&signed_up(id, wiring::at(1_700_000_200)))
            .await
            .expect("messages come at least once, and a withdrawal that crossed a sign-up is no failure");
    }

    assert_eq!(happened(&wired, &accepted).await.len(), 2);
    assert!(matches!(
        happened(&wired, &withdrawn).await.as_slice(),
        [_, InvitationEvent::Withdrawn]
    ));
}

#[tokio::test]
async fn only_a_sign_up_by_invitation_is_heard() {
    let wired = wiring::wired();
    let listener = listener(&wired);

    assert!(listener.hears(&RoutingKey::parse(SIGNED_UP_BY_INVITATION).expect("a declared key")));
    assert!(!listener.hears(&RoutingKey::parse(SIGNED_UP).expect("a declared key")));
}

#[tokio::test]
async fn a_sign_up_naming_no_invitation_is_refused() {
    let wired = wiring::wired();
    let listener = listener(&wired);

    for unreadable in [
        told(
            json!({ "version": 0, "name": "SIGNED_UP_BY_INVITATION", "invitation": "not-an-id" }),
            wiring::at(1_700_000_100),
        ),
        told(
            json!({ "version": 0, "name": "SIGNED_UP" }),
            wiring::at(1_700_000_100),
        ),
        signed_up(
            &InvitationId::generate(wiring::at(1_000)),
            wiring::at(1_700_000_100),
        ),
    ] {
        assert!(
            listener.take_in(&unreadable).await.is_err(),
            "a message the listener cannot act on belongs in dead letters"
        );
    }
}

#[tokio::test]
async fn a_sign_up_told_as_of_after_the_invitation_ran_out_is_refused() {
    let wired = wiring::wired();
    let id = invited(&wired).await;

    assert!(
        listener(&wired)
            .take_in(&signed_up(
                &id,
                wiring::at(1_700_000_000) + Duration::days(7) + Duration::seconds(1),
            ))
            .await
            .is_err(),
        "accounts let nobody in past the last moment, so such a message shows something gone wrong"
    );
}
