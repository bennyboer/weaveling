use acting::Acting;
use eventsourcing::{Agent, AggregateId, EventStore, Version};
use inbox::Inboxed;
use invitations_contract::ISSUED;
use invitations_core::{InvitationCatalog, InvitationEvent, InvitationId, KIND, Role};
use invitations_messaging::{
    CatalogueInvitationsListener, RememberProjectNamesListener, SWEEP_PROJECT, SweepProjectDTO,
    WITHDRAW_IF_PROJECT_GONE, WithdrawIfProjectGoneDTO, WithdrawIfProjectGoneListener,
    WithdrawInvitationsOfDeletedProjectListener,
};
use messaging::{Listener, Message, RoutingKey};
use projects_contract::DELETED;
use serde_json::json;
use transacting::InMemoryTransactions;

use crate::wiring;
use crate::wiring::{ORIGIN, Wired};

fn an_owner() -> Acting {
    Acting {
        agent: Agent::Account("account_ada".into()),
        operator: false,
    }
}

fn an_issue_of(invitation: &InvitationId) -> Message {
    Message::opening(
        RoutingKey::parse(ISSUED).expect("a declared routing key is fine"),
        json!({
            "event": { "version": 0, "name": "WHATEVER" },
            "aggregate": { "id": invitation.to_string(), "kind": "invitation", "version": 1 },
            "agent": { "kind": "system", "id": null },
            "occurred_at": "2023-11-14T22:13:20Z",
        }),
        wiring::at(1_700_000_000),
    )
}

fn a_deletion_of(project: &str) -> Message {
    Message::opening(
        RoutingKey::parse(DELETED).expect("a declared routing key is fine"),
        json!({
            "event": { "version": 0, "name": "DELETED" },
            "aggregate": { "id": project, "kind": "project", "version": 4 },
            "agent": { "kind": "account", "id": "account_ada" },
            "occurred_at": "2023-11-14T22:13:20Z",
        }),
        wiring::at(1_700_000_050),
    )
}

struct Listening {
    wired: Wired,
    cataloguing: Inboxed<CatalogueInvitationsListener>,
    checking: Inboxed<WithdrawIfProjectGoneListener>,
    naming: Inboxed<RememberProjectNamesListener>,
    sweeping: WithdrawInvitationsOfDeletedProjectListener,
}

fn listening() -> Listening {
    let wired = wiring::a_workbench();

    Listening {
        cataloguing: wiring::inboxed(CatalogueInvitationsListener::new(
            wired.invitations.clone(),
            wired.outbox.clone(),
            wired.clock.clone(),
        )),
        checking: wiring::inboxed(WithdrawIfProjectGoneListener::new(
            wired.invitations.clone(),
        )),
        naming: wiring::inboxed(RememberProjectNamesListener::new(
            wired.invitations.clone(),
            wired.outbox.clone(),
            wired.clock.clone(),
        )),
        sweeping: WithdrawInvitationsOfDeletedProjectListener::new(wired.invitations.clone()),
        wired,
    }
}

impl Listening {
    async fn an_open_invitation_into(&self, project: &str) -> InvitationId {
        self.wired
            .invitations
            .invite_to_project(&an_owner(), project, Role::Author, "bo@example.com", ORIGIN)
            .await
            .expect("an owner may invite")
            .id
    }

    async fn hears(&self, listener: &dyn Listener, message: &Message) {
        listener
            .handle(message)
            .await
            .expect("the listener takes it in");
    }

    async fn followed_up(&self) {
        self.wired
            .followed_up_by(&[&self.checking, &self.sweeping])
            .await;
    }

    async fn last_of(&self, id: &InvitationId) -> (InvitationEvent, Agent) {
        let told = self
            .wired
            .store
            .read_from(&AggregateId::from(id), KIND, Version::of(1))
            .await
            .expect("reading should succeed");
        let last = told.last().expect("an invitation has its issue at least");

        (last.event.clone(), last.metadata.agent.clone())
    }
}

#[tokio::test]
async fn an_invitation_into_a_project_once_listed_asks_after_its_project() {
    // given: an open invitation into a project
    let listening = listening();
    let id = listening.an_open_invitation_into("project_loom").await;

    // when: its issue is heard
    listening
        .hears(&listening.cataloguing, &an_issue_of(&id))
        .await;

    // then: it asks after its project
    let follow_ups = listening.wired.follow_ups().await;
    assert_eq!(follow_ups.len(), 1);
    assert_eq!(follow_ups[0].routing.to_string(), WITHDRAW_IF_PROJECT_GONE);
    assert_eq!(
        follow_ups[0]
            .payload_as::<WithdrawIfProjectGoneDTO>()
            .expect("a follow-up is shaped as declared"),
        WithdrawIfProjectGoneDTO {
            invitation: id.to_string(),
            project: "project_loom".to_owned(),
        }
    );
    // and: the list does not hear its own asking
    assert!(
        !listening.cataloguing.hears(&follow_ups[0].routing),
        "every invitation's change is heard, never the list's own follow-up"
    );
}

#[tokio::test]
async fn an_invitation_to_the_instance_asks_after_no_project() {
    // given: an open invitation to the instance
    let listening = listening();
    let id = listening
        .wired
        .invitations
        .invite_to_instance(&wiring::an_operator(), "bo@example.com", ORIGIN)
        .await
        .expect("an operator may invite")
        .id;

    // when: its issue is heard
    listening
        .hears(&listening.cataloguing, &an_issue_of(&id))
        .await;

    // then: nothing is asked
    assert!(listening.wired.follow_ups().await.is_empty());
}

#[tokio::test]
async fn a_listing_whose_hearing_does_not_commit_asks_nothing_and_lists_nothing() {
    // given: an open invitation into a project
    let wired = wiring::a_workbench();
    let id = wired
        .invitations
        .invite_to_project(
            &an_owner(),
            "project_loom",
            Role::Author,
            "bo@example.com",
            ORIGIN,
        )
        .await
        .expect("an owner may invite")
        .id;

    // when: its issue is heard in a hearing that does not commit
    wiring::inboxed_in(
        CatalogueInvitationsListener::new(
            wired.invitations.clone(),
            wired.outbox.clone(),
            wired.clock.clone(),
        ),
        InMemoryTransactions::refusing_commits(),
    )
    .handle(&an_issue_of(&id))
    .await
    .expect_err("a hearing that does not commit is not handled");

    // then: nothing is asked
    assert!(
        wired.follow_ups().await.is_empty(),
        "the follow-up is asked in the hearing's transaction"
    );
    // and: nothing is listed
    assert!(
        wired
            .catalog
            .listed_after(None, 10)
            .await
            .expect("listing should succeed")
            .is_empty(),
        "the summary is written in the hearing's transaction"
    );
}

#[tokio::test]
async fn the_follow_up_withdraws_an_invitation_whose_project_is_gone_and_leaves_one_whose_project_is_not()
 {
    // given: open invitations into two projects
    let listening = listening();
    let into_the_loom = listening.an_open_invitation_into("project_loom").await;
    let into_the_weft = listening.an_open_invitation_into("project_weft").await;
    // and: one project deleted
    listening
        .hears(&listening.naming, &a_deletion_of("project_loom"))
        .await;

    // when: the asking after each project is heard
    for id in [&into_the_loom, &into_the_weft] {
        listening
            .hears(
                &listening.checking,
                &Message::opening(
                    RoutingKey::parse(WITHDRAW_IF_PROJECT_GONE).expect("a declared key"),
                    json!({
                        "invitation": id.to_string(),
                        "project": if id == &into_the_loom { "project_loom" } else { "project_weft" },
                    }),
                    wiring::at(1_700_000_100),
                ),
            )
            .await;
    }

    // then: the one into the deleted project is withdrawn in the system's name
    assert_eq!(
        listening.last_of(&into_the_loom).await,
        (InvitationEvent::Withdrawn, Agent::System)
    );
    // and: the other stays open
    assert!(matches!(
        listening.last_of(&into_the_weft).await.0,
        InvitationEvent::Issued { .. }
    ));
}

#[tokio::test]
async fn a_project_s_tombstone_asks_for_its_invitations_to_be_swept() {
    // given: the listeners following up invitations
    let listening = listening();

    // when: a project's deletion is heard
    listening
        .hears(&listening.naming, &a_deletion_of("project_loom"))
        .await;

    // then: its invitations are asked to be swept
    let follow_ups = listening.wired.follow_ups().await;
    assert_eq!(follow_ups.len(), 1);
    assert_eq!(follow_ups[0].routing.to_string(), SWEEP_PROJECT);
    assert_eq!(
        follow_ups[0]
            .payload_as::<SweepProjectDTO>()
            .expect("a follow-up is shaped as declared"),
        SweepProjectDTO {
            project: "project_loom".to_owned(),
        }
    );
    // and: the sweep hears that asking
    assert!(listening.sweeping.hears(&follow_ups[0].routing));
}

#[tokio::test]
async fn an_invitation_listed_only_after_the_sweep_is_withdrawn_by_its_own_follow_up() {
    // given: an open invitation into a project
    let listening = listening();
    let id = listening.an_open_invitation_into("project_loom").await;

    // and: its project swept before the invitation was listed
    listening
        .hears(&listening.naming, &a_deletion_of("project_loom"))
        .await;
    listening.followed_up().await;
    assert!(
        matches!(
            listening.last_of(&id).await.0,
            InvitationEvent::Issued { .. }
        ),
        "the sweep came before the list had heard of the invitation"
    );
    // when: it is listed and followed up
    listening
        .hears(&listening.cataloguing, &an_issue_of(&id))
        .await;
    listening.followed_up().await;

    // then: it is withdrawn in the system's name
    assert_eq!(
        listening.last_of(&id).await,
        (InvitationEvent::Withdrawn, Agent::System)
    );
}

#[tokio::test]
async fn an_invitation_listed_and_checked_before_the_tombstone_is_withdrawn_by_the_sweep() {
    // given: an open invitation into a project
    let listening = listening();
    let id = listening.an_open_invitation_into("project_loom").await;

    // and: listed and asked after while its project stood
    listening
        .hears(&listening.cataloguing, &an_issue_of(&id))
        .await;
    listening.followed_up().await;
    assert!(
        matches!(
            listening.last_of(&id).await.0,
            InvitationEvent::Issued { .. }
        ),
        "the project was not gone yet when the list asked after it"
    );
    // when: its project's deletion is heard and followed up
    listening
        .hears(&listening.naming, &a_deletion_of("project_loom"))
        .await;
    listening.followed_up().await;

    // then: it is withdrawn in the system's name
    assert_eq!(
        listening.last_of(&id).await,
        (InvitationEvent::Withdrawn, Agent::System)
    );
}
