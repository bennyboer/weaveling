use acting::Acting;
use eventsourcing::{Agent, AggregateId, EventStore, Version};
use invitations_core::{InvitationEvent, InvitationId, InvitationsError, KIND, ProjectLink, Role};
use invitations_messaging::{SWEEP_PROJECT, WithdrawInvitationsOfDeletedProjectListener};
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

async fn a_listed_invitation_into(wired: &Wired, project: &str, to: &str) -> InvitationId {
    let id = wired
        .invitations
        .invite_to_project(&an_owner(), project, Role::Author, to, ORIGIN)
        .await
        .expect("an owner may invite")
        .id;
    wired
        .invitations
        .catalogue(None, &id)
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

fn a_sweep_of(project: &str) -> Message {
    Message::opening(
        RoutingKey::parse(SWEEP_PROJECT).expect("a declared routing key is fine"),
        json!({ "project": project }),
        OffsetDateTime::UNIX_EPOCH,
    )
}

#[tokio::test]
async fn a_deleted_project_s_open_invitations_are_withdrawn_by_the_system_page_after_page() {
    // given: the sweep of a deleted project's invitations
    let wired = wiring::a_workbench();
    let listener = WithdrawInvitationsOfDeletedProjectListener::new(wired.invitations.clone());
    // then: it hears a project's sweep
    assert!(listener.hears(&RoutingKey::parse(SWEEP_PROJECT).expect("a declared key")));
    // and: not its deletion
    assert!(
        !listener.hears(&RoutingKey::parse(DELETED).expect("a declared key")),
        "the sweep waits for the tombstone, which asks for it once it has committed"
    );
    // when: more than a page of invitations into a project and one into another are listed
    let mut into_the_loom = Vec::new();
    for n in 0..101 {
        into_the_loom.push(
            a_listed_invitation_into(&wired, "project_loom", &format!("reader{n}@example.com"))
                .await,
        );
    }
    let elsewhere = a_listed_invitation_into(&wired, "project_weft", "grace@example.com").await;

    // and: the project is swept
    listener
        .take_in(&a_sweep_of("project_loom"))
        .await
        .expect("the sweep goes through");

    // then: each into it is withdrawn by the system
    for id in &into_the_loom {
        assert_eq!(
            last_of(&wired, id).await,
            (InvitationEvent::Withdrawn, Agent::System),
            "every page is swept, not the first alone"
        );
    }
    // and: the other stays open
    assert!(matches!(
        last_of(&wired, &elsewhere).await.0,
        InvitationEvent::Issued { .. }
    ));
}

#[tokio::test]
async fn an_invitation_taken_up_or_withdrawn_before_is_left_as_it_was_and_a_sweep_heard_again_changes_nothing()
 {
    // given: the sweep of a deleted project's invitations
    let wired = wiring::a_workbench();
    let listener = WithdrawInvitationsOfDeletedProjectListener::new(wired.invitations.clone());
    // and: an invitation into the project taken up and one open
    let accepted = a_listed_invitation_into(&wired, "project_loom", "ada@example.com").await;
    let open = a_listed_invitation_into(&wired, "project_loom", "grace@example.com").await;
    wired
        .invitations
        .taken_up(None, &accepted, &Agent::Account("account_ada".into()))
        .await
        .expect("taking up should succeed");

    // when: the project is swept twice
    for _ in 0..2 {
        listener
            .take_in(&a_sweep_of("project_loom"))
            .await
            .expect("an invitation accepted meanwhile, or the sweep heard again, is no failure");
    }

    // then: the one taken up stays accepted
    assert!(matches!(
        last_of(&wired, &accepted).await.0,
        InvitationEvent::Accepted { .. }
    ));
    // and: the open one is withdrawn once
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
    // given: an invitation into a project not yet listed
    let wired = wiring::a_workbench();
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
    // and: the project swept and its tombstone laid
    WithdrawInvitationsOfDeletedProjectListener::new(wired.invitations.clone())
        .take_in(&a_sweep_of("project_loom"))
        .await
        .expect("the sweep goes through");
    wired
        .invitations
        .lay_project_tombstone(
            None,
            &ProjectLink::from("project_loom"),
            OffsetDateTime::UNIX_EPOCH,
        )
        .await
        .expect("laying the tombstone should succeed");

    // then: no invitation into it is issued
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
    // and: the one issued before is missed
    assert!(
        matches!(
            last_of(&wired, &just_before).await.0,
            InvitationEvent::Issued { .. }
        ),
        "the sweep missed it, as the list had not heard of it yet"
    );

    // when: that one is listed and asked after
    let project = wired
        .invitations
        .catalogue(None, &just_before)
        .await
        .expect("cataloguing should succeed")
        .expect("an invitation into a project asks after its project");
    wired
        .invitations
        .withdraw_if_project_gone(None, &just_before, &project)
        .await
        .expect("withdrawing should succeed");

    // then: it is withdrawn by the system
    assert_eq!(
        last_of(&wired, &just_before).await,
        (InvitationEvent::Withdrawn, Agent::System)
    );
    // and: invitations into other projects are still issued
    a_listed_invitation_into(&wired, "project_weft", "hedy@example.com").await;
}

#[tokio::test]
async fn an_invitation_into_a_deleted_project_taken_up_before_is_listed_as_it_was() {
    // given: a listed invitation into a project, taken up
    let wired = wiring::a_workbench();
    let accepted = a_listed_invitation_into(&wired, "project_loom", "ada@example.com").await;
    wired
        .invitations
        .taken_up(None, &accepted, &Agent::Account("account_ada".into()))
        .await
        .expect("taking up should succeed");
    // and: the project's tombstone
    wired
        .invitations
        .lay_project_tombstone(
            None,
            &ProjectLink::from("project_loom"),
            OffsetDateTime::UNIX_EPOCH,
        )
        .await
        .expect("laying the tombstone should succeed");

    // when: the invitation is listed and asked after again
    wired
        .invitations
        .catalogue(None, &accepted)
        .await
        .expect("an invitation taken up is no failure to list");
    wired
        .invitations
        .withdraw_if_project_gone(None, &accepted, &ProjectLink::from("project_loom"))
        .await
        .expect("an invitation taken up is no failure to withdraw if its project is gone");

    // then: it stays accepted
    assert!(matches!(
        last_of(&wired, &accepted).await.0,
        InvitationEvent::Accepted { .. }
    ));
}
