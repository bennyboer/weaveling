use accounts_contract::{INVITATION_TAKEN_UP, SIGNED_UP_BY_INVITATION};
use eventsourcing::{AggregateId, EventStore, Version};
use invitations_core::{InvitationEvent, InvitationId, KIND, ProjectLink, Role, Scope};
use invitations_messaging::AcceptTakenUpInvitationListener;
use messaging::{Handler, Listener, Message, RoutingKey};
use serde_json::{Value, json};
use time::{Duration, OffsetDateTime};

use crate::wiring;
use crate::wiring::{ORIGIN, Wired};

const ADA: &str = "ada@example.com";

fn taken_up(invitation: &InvitationId, at: OffsetDateTime) -> Message {
    told(
        json!({ "version": 0, "name": "INVITATION_TAKEN_UP", "invitation": invitation.to_string() }),
        at,
    )
}

fn told(event: Value, at: OffsetDateTime) -> Message {
    Message::opening(
        RoutingKey::parse(INVITATION_TAKEN_UP).expect("a declared routing key is fine"),
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
        .invite_to_instance(&wiring::an_operator(), ADA, ORIGIN)
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

fn listener(wired: &Wired) -> AcceptTakenUpInvitationListener {
    AcceptTakenUpInvitationListener::new(wired.invitations.clone())
}

#[tokio::test]
async fn an_invitation_an_account_took_up_is_accepted() {
    let wired = wiring::wired();
    let id = invited(&wired).await;

    listener(&wired)
        .take_in(&taken_up(&id, wiring::at(1_700_000_100)))
        .await
        .expect("an open invitation is accepted");

    assert!(matches!(
        happened(&wired, &id).await.as_slice(),
        [
            InvitationEvent::Issued { .. },
            InvitationEvent::Accepted { .. }
        ]
    ));
}

#[tokio::test]
async fn an_invitation_is_accepted_as_of_its_taking_up_however_late_it_is_heard() {
    let wired = wiring::wired();
    let id = invited(&wired).await;
    wired
        .clock
        .set(wiring::at(1_700_000_000) + Duration::days(30));

    listener(&wired)
        .take_in(&taken_up(
            &id,
            wiring::at(1_700_000_000) + Duration::days(7),
        ))
        .await
        .expect("the account was made while the invitation held");

    assert!(matches!(
        happened(&wired, &id).await.as_slice(),
        [_, InvitationEvent::Accepted { .. }]
    ));
}

#[tokio::test]
async fn a_taking_up_told_again_or_after_a_withdrawal_changes_nothing() {
    let wired = wiring::wired();
    let accepted = invited(&wired).await;
    let withdrawn = invited(&wired).await;
    wired
        .invitations
        .withdraw(&wiring::an_operator(), &withdrawn)
        .await
        .expect("an operator may withdraw");
    let listener = listener(&wired);
    listener
        .take_in(&taken_up(&accepted, wiring::at(1_700_000_100)))
        .await
        .expect("an open invitation is accepted");

    for id in [&accepted, &withdrawn] {
        listener
            .take_in(&taken_up(id, wiring::at(1_700_000_200)))
            .await
            .expect("messages come at least once, and a withdrawal that crossed a taking up is no failure");
    }

    assert_eq!(happened(&wired, &accepted).await.len(), 2);
    assert!(matches!(
        happened(&wired, &withdrawn).await.as_slice(),
        [_, InvitationEvent::Withdrawn]
    ));
}

#[tokio::test]
async fn only_an_invitation_taken_up_is_heard() {
    let wired = wiring::wired();
    let listener = listener(&wired);

    assert!(listener.hears(&RoutingKey::parse(INVITATION_TAKEN_UP).expect("a declared key")));
    assert!(
        !listener.hears(&RoutingKey::parse(SIGNED_UP_BY_INVITATION).expect("a declared key")),
        "a sign-up by invitation is told as taken up too, so hearing both would accept twice"
    );
}

#[tokio::test]
async fn a_taking_up_naming_no_invitation_is_refused() {
    let wired = wiring::wired();
    let listener = listener(&wired);

    for unreadable in [
        told(
            json!({ "version": 0, "name": "INVITATION_TAKEN_UP", "invitation": "not-an-id" }),
            wiring::at(1_700_000_100),
        ),
        told(
            json!({ "version": 0, "name": "SIGNED_UP_BY_INVITATION" }),
            wiring::at(1_700_000_100),
        ),
        taken_up(
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
async fn a_taking_up_told_as_of_after_the_invitation_ran_out_is_refused() {
    let wired = wiring::wired();
    let id = invited(&wired).await;

    assert!(
        listener(&wired)
            .take_in(&taken_up(
                &id,
                wiring::at(1_700_000_000) + Duration::days(7) + Duration::seconds(1),
            ))
            .await
            .is_err(),
        "accounts let nobody in past the last moment, so such a message shows something gone wrong"
    );
}

#[tokio::test]
async fn an_invitation_into_a_project_is_accepted_telling_the_project_and_role() {
    let wired = wiring::wired();
    let id = wired
        .invitations
        .invite_to_project(
            &wiring::an_operator(),
            "project_loom",
            Role::Author,
            ADA,
            ORIGIN,
        )
        .await
        .expect("whoever may manage the project may invite")
        .id;

    listener(&wired)
        .take_in(&taken_up(&id, wiring::at(1_700_000_100)))
        .await
        .expect("an open invitation is accepted");

    assert_eq!(
        happened(&wired, &id).await.last(),
        Some(&InvitationEvent::Accepted {
            scope: Scope::Project {
                project: ProjectLink::from("project_loom"),
                role: Role::Author,
            },
        })
    );
}
