use accounts_contract::{INVITATION_DECLINED, SIGNED_UP_BY_INVITATION};
use eventsourcing::{Agent, AggregateId, EventStore, Version};
use inbox::Inboxed;
use invitations_core::{DeclineReason, InvitationEvent, InvitationId, KIND};
use invitations_messaging::DeclineInvitationListener;
use messaging::{Listener, Message, RoutingKey};
use serde_json::{Value, json};
use transacting::InMemoryTransactions;

use crate::wiring;
use crate::wiring::{ORIGIN, Wired};

const ADA: &str = "ada@example.com";

fn a_declining(invitation: &InvitationId) -> Message {
    a_declining_for(invitation, "address-has-account")
}

fn a_declining_for(invitation: &InvitationId, reason: &str) -> Message {
    a_message(
        json!({ "version": 0, "name": "INVITATION_DECLINED", "invitation": invitation.to_string(), "reason": reason }),
    )
}

fn a_message(event: Value) -> Message {
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

async fn an_open_invitation(wired: &Wired) -> InvitationId {
    wired
        .invitations
        .invite_to_instance(&wiring::an_operator(), ADA, ORIGIN)
        .await
        .expect("an operator may invite")
        .id
}

async fn events_of(wired: &Wired, id: &InvitationId) -> Vec<InvitationEvent> {
    wired
        .store
        .read_from(&AggregateId::from(id), KIND, Version::of(1))
        .await
        .expect("reading should succeed")
        .into_iter()
        .map(|recorded| recorded.event)
        .collect()
}

fn listener(wired: &Wired) -> Inboxed<DeclineInvitationListener> {
    wiring::inboxed(DeclineInvitationListener::new(wired.invitations.clone()))
}

#[tokio::test]
async fn an_invitation_is_declined_in_the_system_s_name() {
    // given: an open invitation
    let wired = wiring::a_workbench();
    let id = an_open_invitation(&wired).await;

    // when: its declining is heard
    listener(&wired)
        .handle(&a_declining(&id))
        .await
        .expect("declining should succeed");

    // then: it is declined in the system's name
    let recorded = wired
        .store
        .read_from(&AggregateId::from(&id), KIND, Version::of(1))
        .await
        .expect("reading should succeed");
    assert_eq!(
        recorded.last().map(|recorded| &recorded.metadata.agent),
        Some(&Agent::System)
    );
}

#[tokio::test]
async fn a_declining_whose_hearing_does_not_commit_is_undone_with_it() {
    // given: an open invitation
    let wired = wiring::a_workbench();
    let id = an_open_invitation(&wired).await;

    // when: its declining is heard in a hearing that does not commit
    wiring::inboxed_in(
        DeclineInvitationListener::new(wired.invitations.clone()),
        InMemoryTransactions::refusing_commits(),
    )
    .handle(&a_declining(&id))
    .await
    .expect_err("a hearing that does not commit is not handled");

    // then: it stays open
    assert!(
        matches!(
            events_of(&wired, &id).await.as_slice(),
            [InvitationEvent::Issued { .. }]
        ),
        "the declining is written in the hearing's transaction"
    );
}

#[tokio::test]
async fn an_invitation_declined_by_an_account_is_declined_once() {
    // given: an open invitation
    let wired = wiring::a_workbench();
    let id = an_open_invitation(&wired).await;
    let listener = listener(&wired);

    // when: its declining is heard twice
    for _ in 0..2 {
        listener
            .handle(&a_declining(&id))
            .await
            .expect("messages come at least once");
    }

    // then: it is declined once
    assert!(matches!(
        events_of(&wired, &id).await.as_slice(),
        [
            InvitationEvent::Issued { .. },
            InvitationEvent::Declined {
                reason: DeclineReason::AddressHasAccount
            }
        ]
    ));
}

#[tokio::test]
async fn an_invitation_turned_down_by_whom_it_was_for_is_declined_so() {
    // given: an open invitation
    let wired = wiring::a_workbench();
    let id = an_open_invitation(&wired).await;

    // when: its turning down by whom it was for is heard
    listener(&wired)
        .handle(&a_declining_for(&id, "turned-down"))
        .await
        .expect("declining should succeed");

    // then: it is declined as turned down
    assert!(matches!(
        events_of(&wired, &id).await.as_slice(),
        [
            InvitationEvent::Issued { .. },
            InvitationEvent::Declined {
                reason: DeclineReason::TurnedDown
            }
        ]
    ));
}

#[tokio::test]
async fn an_invitation_withdrawn_or_taken_up_before_it_was_declined_stays_so() {
    // given: a withdrawn invitation
    let wired = wiring::a_workbench();
    let withdrawn = an_open_invitation(&wired).await;
    // and: an accepted one
    let accepted = an_open_invitation(&wired).await;
    wired
        .invitations
        .withdraw(&wiring::an_operator(), &withdrawn)
        .await
        .expect("an operator may withdraw");
    wired
        .invitations
        .taken_up(None, &accepted, &Agent::Account("account_ada".into()))
        .await
        .expect("an open invitation is accepted");
    let listener = listener(&wired);

    // when: a declining of each is heard
    for id in [&withdrawn, &accepted] {
        listener
            .handle(&a_declining(id))
            .await
            .expect("what is settled already is no failure");
    }

    // then: the withdrawn one stays withdrawn
    assert!(matches!(
        events_of(&wired, &withdrawn).await.as_slice(),
        [_, InvitationEvent::Withdrawn]
    ));
    // and: the accepted one stays accepted
    assert!(matches!(
        events_of(&wired, &accepted).await.as_slice(),
        [_, InvitationEvent::Accepted { .. }]
    ));
}

#[tokio::test]
async fn only_a_declined_invitation_is_heard() {
    // given: the listener declining invitations
    let wired = wiring::a_workbench();
    let listener = listener(&wired);

    // then: it hears a declined invitation
    assert!(listener.hears(&RoutingKey::parse(INVITATION_DECLINED).expect("a declared key")));
    // and: not a sign-up by invitation
    assert!(!listener.hears(&RoutingKey::parse(SIGNED_UP_BY_INVITATION).expect("a declared key")));
}

#[tokio::test]
async fn a_decline_naming_no_invitation_or_no_reason_is_refused() {
    // given: an open invitation
    let wired = wiring::a_workbench();
    let open = an_open_invitation(&wired).await;
    let listener = listener(&wired);

    // when: a declining it cannot act on is heard
    for unreadable in [
        a_message(
            json!({ "version": 0, "name": "INVITATION_DECLINED", "invitation": "not-an-id", "reason": "address-has-account" }),
        ),
        a_message(
            json!({ "version": 0, "name": "INVITATION_DECLINED", "invitation": open.to_string(), "reason": "had a bad day" }),
        ),
        a_message(
            json!({ "version": 0, "name": "INVITATION_DECLINED", "invitation": open.to_string() }),
        ),
        a_message(
            json!({ "version": 0, "name": "SIGNED_UP", "personal": { "name": "Ada", "email": "ada@example.com" } }),
        ),
        a_declining(&InvitationId::generate(wiring::at(1_000))),
    ] {
        // then: it is refused
        assert!(
            listener.handle(&unreadable).await.is_err(),
            "a message the listener cannot act on belongs in dead letters"
        );
    }
    // and: the invitation stays open
    assert_eq!(
        events_of(&wired, &open).await.len(),
        1,
        "a reason nobody declared declines nothing"
    );
}
