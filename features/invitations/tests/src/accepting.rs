use accounts_contract::{INVITATION_TAKEN_UP, SIGNED_UP_BY_INVITATION};
use eventsourcing::{AccountId, Agent, AggregateId, EventStore, Version};
use inbox::Inboxed;
use invitations_core::{
    DeclineReason, InvitationEvent, InvitationId, KIND, ProjectLink, Role, Scope,
};
use invitations_messaging::AcceptTakenUpInvitationListener;
use messaging::{Listener, Message, RoutingKey};
use serde_json::{Value, json};
use time::{Duration, OffsetDateTime};

use crate::wiring;
use crate::wiring::{ORIGIN, Wired};

const ADA: &str = "ada@example.com";

fn a_taking_up(invitation: &InvitationId, at: OffsetDateTime) -> Message {
    a_message(
        json!({ "version": 0, "name": "INVITATION_TAKEN_UP", "invitation": invitation.to_string() }),
        at,
    )
}

fn a_message(event: Value, at: OffsetDateTime) -> Message {
    eventpublishing::published(
        INVITATION_TAKEN_UP,
        "account",
        "account_ada",
        1,
        event,
        &Agent::Account(AccountId::from("account_ada")),
        at,
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

fn listener(wired: &Wired) -> Inboxed<AcceptTakenUpInvitationListener> {
    wiring::inboxed(AcceptTakenUpInvitationListener::new(
        wired.invitations.clone(),
    ))
}

#[tokio::test]
async fn an_invitation_an_account_took_up_is_accepted() {
    // given: an open invitation
    let wired = wiring::a_workbench();
    let id = an_open_invitation(&wired).await;

    // when: its taking up is heard
    listener(&wired)
        .handle(&a_taking_up(&id, clock::at(1_700_000_100)))
        .await
        .expect("an open invitation is accepted");

    // then: it is accepted
    assert!(matches!(
        events_of(&wired, &id).await.as_slice(),
        [
            InvitationEvent::Issued { .. },
            InvitationEvent::Accepted { .. }
        ]
    ));
}

#[tokio::test]
async fn an_invitation_is_accepted_as_of_its_taking_up_however_late_it_is_heard() {
    // given: an open invitation
    let wired = wiring::a_workbench();
    let id = an_open_invitation(&wired).await;
    // and: its last moment long past
    wired
        .clock
        .set(clock::at(1_700_000_000) + Duration::days(30));

    // when: a taking up from its last moment is heard
    listener(&wired)
        .handle(&a_taking_up(
            &id,
            clock::at(1_700_000_000) + Duration::days(7),
        ))
        .await
        .expect("the account was made while the invitation held");

    // then: it is accepted
    assert!(matches!(
        events_of(&wired, &id).await.as_slice(),
        [_, InvitationEvent::Accepted { .. }]
    ));
}

#[tokio::test]
async fn a_taking_up_told_again_or_after_a_withdrawal_changes_nothing() {
    // given: an accepted invitation
    let wired = wiring::a_workbench();
    let accepted = an_open_invitation(&wired).await;
    // and: a withdrawn one
    let withdrawn = an_open_invitation(&wired).await;
    wired
        .invitations
        .withdraw(&wiring::an_operator(), &withdrawn, None)
        .await
        .expect("an operator may withdraw");
    let listener = listener(&wired);
    listener
        .handle(&a_taking_up(&accepted, clock::at(1_700_000_100)))
        .await
        .expect("an open invitation is accepted");

    // when: a taking up of each is heard
    for id in [&accepted, &withdrawn] {
        listener
            .handle(&a_taking_up(id, clock::at(1_700_000_200)))
            .await
            .expect("messages come at least once, and a withdrawal that crossed a taking up is no failure");
    }

    // then: the accepted one is accepted once
    assert_eq!(events_of(&wired, &accepted).await.len(), 2);
    // and: the withdrawn one stays withdrawn
    assert!(matches!(
        events_of(&wired, &withdrawn).await.as_slice(),
        [_, InvitationEvent::Withdrawn]
    ));
}

#[tokio::test]
async fn a_taking_up_that_crossed_a_decline_by_link_changes_nothing() {
    // given: an invitation declined by link
    let wired = wiring::a_workbench();
    let id = an_open_invitation(&wired).await;
    wired
        .invitations
        .declined(None, &id, DeclineReason::TurnedDown)
        .await
        .expect("an open invitation is declined");

    // when: its taking up is heard
    listener(&wired)
        .handle(&a_taking_up(&id, clock::at(1_700_000_100)))
        .await
        .expect(
            "declining by link claims no ticket, so a taking up may cross it, and no retry would ever accept",
        );

    // then: it stays declined
    assert!(matches!(
        events_of(&wired, &id).await.as_slice(),
        [_, InvitationEvent::Declined { .. }]
    ));
}

#[tokio::test]
async fn only_an_invitation_taken_up_is_heard() {
    // given: the listener accepting taken up invitations
    let wired = wiring::a_workbench();
    let listener = listener(&wired);

    // then: it hears an invitation taken up
    assert!(listener.hears(&RoutingKey::parse(INVITATION_TAKEN_UP).expect("a declared key")));
    // and: not a sign-up by invitation
    assert!(
        !listener.hears(&RoutingKey::parse(SIGNED_UP_BY_INVITATION).expect("a declared key")),
        "a sign-up by invitation is told as taken up too, so hearing both would accept twice"
    );
}

#[tokio::test]
async fn a_taking_up_naming_no_invitation_is_refused() {
    // given: the listener accepting taken up invitations
    let wired = wiring::a_workbench();
    let listener = listener(&wired);

    // when: a taking up it cannot act on is heard
    for unreadable in [
        a_message(
            json!({ "version": 0, "name": "INVITATION_TAKEN_UP", "invitation": "not-an-id" }),
            clock::at(1_700_000_100),
        ),
        a_message(
            json!({ "version": 0, "name": "SIGNED_UP_BY_INVITATION", "personal": { "name": "Ada", "email": "ada@example.com" } }),
            clock::at(1_700_000_100),
        ),
        a_taking_up(
            &InvitationId::generate(clock::at(1_000)),
            clock::at(1_700_000_100),
        ),
    ] {
        // then: it is refused
        assert!(
            listener.handle(&unreadable).await.is_err(),
            "a message the listener cannot act on belongs in dead letters"
        );
    }
}

#[tokio::test]
async fn a_taking_up_stamped_after_the_last_moment_is_accepted_as_accounts_decided() {
    // given: an open invitation
    let wired = wiring::a_workbench();
    let id = an_open_invitation(&wired).await;

    // when: a taking up stamped just after its last moment is heard
    listener(&wired)
        .handle(&a_taking_up(
            &id,
            clock::at(1_700_000_000) + Duration::days(7) + Duration::seconds(1),
        ))
        .await
        .expect("accounts checked the ticket in time and stamped its taking up a moment later");

    // then: it is accepted
    assert!(matches!(
        events_of(&wired, &id).await.as_slice(),
        [_, InvitationEvent::Accepted { .. }]
    ));
}

#[tokio::test]
async fn an_invitation_into_a_project_is_accepted_telling_the_project_and_role() {
    // given: an open invitation into a project
    let wired = wiring::a_workbench();
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

    // when: its taking up is heard
    listener(&wired)
        .handle(&a_taking_up(&id, clock::at(1_700_000_100)))
        .await
        .expect("an open invitation is accepted");

    // then: it is accepted into that project with that role
    assert_eq!(
        events_of(&wired, &id).await.last(),
        Some(&InvitationEvent::Accepted {
            scope: Scope::Project {
                project: ProjectLink::from("project_loom"),
                role: Role::Author,
            },
        })
    );
}
