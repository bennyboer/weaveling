use acting::Acting;
use eventsourcing::{Agent, AggregateId, EventStore, Version};
use invitations_core::{InvitationEvent, InvitationId, KIND, Role};
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
