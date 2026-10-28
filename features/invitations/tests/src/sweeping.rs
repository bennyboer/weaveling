use acting::Acting;
use eventsourcing::{Agent, AggregateId, EventStore, Version};
use invitations_core::{InvitationEvent, InvitationId, InvitationsError, KIND, ProjectLink, Role};
use invitations_messaging::WithdrawInvitationsOfDeletedProjectListener;
use messaging::{Handler, Listener, Message, RoutingKey};
use projects_contract::DELETED;
use serde_json::json;
use time::OffsetDateTime;

use crate::wiring;
use crate::wiring::{ORIGIN, Wired};

fn an_owner() -> Acting {
    Acting {
        agent: Agent::Account("account_ada".into()),
        operator: false,
    }
}

async fn invited_into(wired: &Wired, project: &str, to: &str) -> InvitationId {
    let id = wired
        .invitations
        .invite_to_project(&an_owner(), project, Role::Author, to, ORIGIN)
        .await
        .expect("an owner may invite")
        .id;
    wired
        .invitations
        .catalogue(&id)
        .await
        .expect("cataloguing should succeed");

    id
}

async fn last_of(wired: &Wired, id: &InvitationId) -> (InvitationEvent, Agent) {
    let told = wired
        .store
        .read_from(&AggregateId::from(id), KIND, Version::of(1))
        .await
        .expect("reading should succeed");
    let last = told.last().expect("an invitation has its issue at least");

    (last.event.clone(), last.metadata.agent.clone())
}

fn deleted(project: &str) -> Message {
    Message::opening(
        RoutingKey::parse(DELETED).expect("a declared routing key is fine"),
        json!({
            "event": { "version": 0, "name": "DELETED" },
            "aggregate": { "id": project, "kind": "project", "version": 4 },
            "agent": { "kind": "account", "id": "account_ada" },
            "occurred_at": "2023-11-14T22:13:20Z",
        }),
        OffsetDateTime::UNIX_EPOCH,
    )
}

#[tokio::test]
async fn a_deleted_project_s_open_invitations_are_withdrawn_by_the_system_page_after_page() {
    let wired = wiring::wired();
    let listener = WithdrawInvitationsOfDeletedProjectListener::new(wired.invitations.clone());
    assert!(listener.hears(&RoutingKey::parse(DELETED).expect("a declared key")));
    let mut into_the_loom = Vec::new();
    for n in 0..101 {
        into_the_loom
            .push(invited_into(&wired, "project_loom", &format!("reader{n}@example.com")).await);
    }
    let elsewhere = invited_into(&wired, "project_weft", "grace@example.com").await;

    listener
        .take_in(&deleted("project_loom"))
        .await
        .expect("the sweep goes through");

    for id in &into_the_loom {
        assert_eq!(
            last_of(&wired, id).await,
            (InvitationEvent::Withdrawn, Agent::System),
            "every page is swept, not the first alone"
        );
    }
    assert!(matches!(
        last_of(&wired, &elsewhere).await.0,
        InvitationEvent::Issued { .. }
    ));
}

#[tokio::test]
async fn an_invitation_taken_up_or_withdrawn_before_is_left_as_it_was_and_a_sweep_heard_again_changes_nothing()
 {
    let wired = wiring::wired();
    let listener = WithdrawInvitationsOfDeletedProjectListener::new(wired.invitations.clone());
    let accepted = invited_into(&wired, "project_loom", "ada@example.com").await;
    let open = invited_into(&wired, "project_loom", "grace@example.com").await;
    wired
        .invitations
        .taken_up(
            &accepted,
            wiring::at(1_700_000_100),
            &Agent::Account("account_ada".into()),
        )
        .await
        .expect("taking up should succeed");

    for _ in 0..2 {
        listener
            .take_in(&deleted("project_loom"))
            .await
            .expect("an invitation accepted meanwhile, or the sweep heard again, is no failure");
    }

    assert!(matches!(
        last_of(&wired, &accepted).await.0,
        InvitationEvent::Accepted { .. }
    ));
    let told = wired
        .store
        .read_from(&AggregateId::from(&open), KIND, Version::of(1))
        .await
        .expect("reading should succeed");
    assert_eq!(
        told.iter()
            .filter(|recorded| recorded.event == InvitationEvent::Withdrawn)
            .count(),
        1
    );
}

#[tokio::test]
async fn into_a_deleted_project_no_invitation_is_issued_and_one_issued_just_before_is_withdrawn_once_listed()
 {
    let wired = wiring::wired();
    let just_before = wired
        .invitations
        .invite_to_project(
            &an_owner(),
            "project_loom",
            Role::Author,
            "ada@example.com",
            ORIGIN,
        )
        .await
        .expect("an owner may invite")
        .id;
    WithdrawInvitationsOfDeletedProjectListener::new(wired.invitations.clone())
        .take_in(&deleted("project_loom"))
        .await
        .expect("the sweep goes through");
    wired
        .invitations
        .lay_project_tombstone(
            &ProjectLink::from("project_loom"),
            OffsetDateTime::UNIX_EPOCH,
        )
        .await
        .expect("laying the tombstone should succeed");

    assert!(matches!(
        wired
            .invitations
            .invite_to_project(
                &an_owner(),
                "project_loom",
                Role::Author,
                "grace@example.com",
                ORIGIN
            )
            .await,
        Err(InvitationsError::ProjectNotThere)
    ));
    assert!(
        matches!(
            last_of(&wired, &just_before).await.0,
            InvitationEvent::Issued { .. }
        ),
        "the sweep missed it, as the list had not heard of it yet"
    );

    wired
        .invitations
        .catalogue(&just_before)
        .await
        .expect("cataloguing should succeed");

    assert_eq!(
        last_of(&wired, &just_before).await,
        (InvitationEvent::Withdrawn, Agent::System)
    );
    invited_into(&wired, "project_weft", "hedy@example.com").await;
}

#[tokio::test]
async fn an_invitation_into_a_deleted_project_taken_up_before_is_listed_as_it_was() {
    let wired = wiring::wired();
    let accepted = invited_into(&wired, "project_loom", "ada@example.com").await;
    wired
        .invitations
        .taken_up(
            &accepted,
            wiring::at(1_700_000_100),
            &Agent::Account("account_ada".into()),
        )
        .await
        .expect("taking up should succeed");
    wired
        .invitations
        .lay_project_tombstone(
            &ProjectLink::from("project_loom"),
            OffsetDateTime::UNIX_EPOCH,
        )
        .await
        .expect("laying the tombstone should succeed");

    wired
        .invitations
        .catalogue(&accepted)
        .await
        .expect("an invitation taken up is no failure to list");

    assert!(matches!(
        last_of(&wired, &accepted).await.0,
        InvitationEvent::Accepted { .. }
    ));
}
