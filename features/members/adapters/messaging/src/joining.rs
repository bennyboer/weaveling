use async_trait::async_trait;
use inbox::KeptHandler;
use invitations_contract::{ACCEPTED, InvitationEventDTO, ScopeDTO, role};
use members_core::{MembersCore, Role};
use messaging::{Failure, Message};
use transacting::Transaction;

const AN_ACCOUNT: &str = "account";

pub struct JoinByAcceptedInvitationListener {
    members: MembersCore,
}

impl JoinByAcceptedInvitationListener {
    pub fn new(members: MembersCore) -> Self {
        Self { members }
    }
}

#[async_trait]
impl KeptHandler for JoinByAcceptedInvitationListener {
    const NAME: &'static str = "join-by-accepted-invitation";
    const LISTENS_TO: &'static [&'static str] = &[ACCEPTED];
    const WHEN_REFUSED: &'static str = "Someone who took up an invitation into a project did not join it, so it stays out of their reach.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let accepted = eventpublishing::published_in::<InvitationEventDTO>(message)?;
        let InvitationEventDTO::Accepted { scope } = accepted.event.body else {
            return Err("an acceptance tells no scope".into());
        };
        let ScopeDTO::Project { project, role } = scope else {
            return Ok(());
        };
        let (AN_ACCOUNT, Some(account)) = (accepted.agent.kind.as_str(), accepted.agent.id) else {
            return Err("an invitation into a project was accepted by no account".into());
        };
        let role = to_role(&role).ok_or_else(|| format!("{role:?} is no role"))?;

        Ok(self
            .members
            .join(Some(transaction), &project, &account, role)
            .await?)
    }
}

fn to_role(named: &str) -> Option<Role> {
    match named {
        role::OWNER => Some(Role::Owner),
        role::AUTHOR => Some(Role::Author),
        role::READER => Some(Role::Reader),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use authorizing::AllowingEverything;
    use clock::FixedClock;
    use eventsourcing::{AccountId, AggregateId, EventStore, InMemoryEventStore, Version};
    use members_catalog::InMemoryMembershipCatalog;
    use members_core::{KIND, MembersEvent, ProjectLink};
    use messaging::{Listener, RoutingKey};
    use serde_json::{Value, json};
    use time::OffsetDateTime;
    use transacting::InMemoryTransactions;

    use inbox::{InMemoryInbox, Inboxed};

    use super::*;

    struct Bench {
        store: Arc<InMemoryEventStore<MembersEvent>>,
        members: MembersCore,
        listener: Inboxed<JoinByAcceptedInvitationListener>,
    }

    async fn a_founded_project() -> Bench {
        let store = Arc::new(InMemoryEventStore::new());
        let members = MembersCore::new(
            store.clone(),
            Arc::new(InMemoryTransactions::new()),
            Arc::new(InMemoryMembershipCatalog::new()),
            Arc::new(AllowingEverything),
            Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        );
        members
            .found(None, "project_1", "account_ada")
            .await
            .expect("founding should succeed");

        Bench {
            listener: Inboxed::new(
                JoinByAcceptedInvitationListener::new(members.clone()),
                Arc::new(InMemoryTransactions::new()),
                Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(
                    OffsetDateTime::UNIX_EPOCH,
                )))),
            ),
            members,
            store,
        }
    }

    fn an_acceptance(scope: Value, agent: Value) -> Message {
        Message::opening(
            RoutingKey::parse(ACCEPTED).expect("a declared key"),
            json!({
                "event": { "version": 0, "name": "ACCEPTED", "scope": scope },
                "aggregate": { "id": "invitation_1", "kind": "invitation", "version": 2 },
                "agent": agent,
                "occurred_at": "1970-01-01T00:00:00Z",
            }),
            OffsetDateTime::UNIX_EPOCH,
        )
    }

    fn into_the_project_as(role: &str) -> Value {
        json!({ "kind": "project", "project": "project_1", "role": role })
    }

    fn bo() -> Value {
        json!({ "kind": "account", "id": "account_bo" })
    }

    async fn events_after_founding(bench: &Bench) -> Vec<MembersEvent> {
        bench
            .store
            .read_from(
                &AggregateId::from(&ProjectLink::from("project_1")),
                KIND,
                Version::of(2),
            )
            .await
            .expect("reading should succeed")
            .into_iter()
            .map(|recorded| recorded.event)
            .collect()
    }

    #[tokio::test]
    async fn whoever_accepted_joins_the_project_in_the_role_invited_however_often_it_is_told() {
        for (named, role) in [
            ("owner", Role::Owner),
            ("author", Role::Author),
            ("reader", Role::Reader),
        ] {
            // given: a founded project
            let bench = a_founded_project().await;

            // when: an acceptance of an invitation into it in each role is heard twice
            for _ in 0..2 {
                bench
                    .listener
                    .handle(&an_acceptance(into_the_project_as(named), bo()))
                    .await
                    .expect("messages come at least once");
            }

            // then: whoever accepted joined once, in that role
            assert_eq!(
                events_after_founding(&bench).await,
                vec![MembersEvent::Joined {
                    account: AccountId::from("account_bo"),
                    role,
                }]
            );
        }
    }

    #[tokio::test]
    async fn a_member_already_keeps_the_role_they_have() {
        // given: a founded project
        let bench = a_founded_project().await;
        // and: a reader of it
        bench
            .members
            .join(None, "project_1", "account_bo", Role::Reader)
            .await
            .expect("joining should succeed");

        // when: their acceptance of an invitation as author is heard
        bench
            .listener
            .handle(&an_acceptance(into_the_project_as("author"), bo()))
            .await
            .expect("an owner changes a member's role; an invitation is no way around that");

        // then: they stay a reader
        assert_eq!(
            events_after_founding(&bench).await,
            vec![MembersEvent::Joined {
                account: AccountId::from("account_bo"),
                role: Role::Reader,
            }]
        );
    }

    #[tokio::test]
    async fn an_invitation_to_the_instance_lets_nobody_join_anything() {
        // given: a founded project
        let bench = a_founded_project().await;

        // when: an acceptance of an invitation to the instance is heard
        bench
            .listener
            .handle(&an_acceptance(json!({ "kind": "instance" }), bo()))
            .await
            .expect("there is nothing to join");

        // then: nobody joined
        assert!(events_after_founding(&bench).await.is_empty());
    }

    #[tokio::test]
    async fn an_acceptance_the_listener_cannot_act_on_belongs_in_dead_letters() {
        // given: a founded project
        let bench = a_founded_project().await;

        // expect: hearing each acceptance with no role, no account or no sense fails
        for unreadable in [
            an_acceptance(into_the_project_as("editor"), bo()),
            an_acceptance(
                into_the_project_as("author"),
                json!({ "kind": "system", "id": null }),
            ),
            an_acceptance(
                into_the_project_as("author"),
                json!({ "kind": "account", "id": null }),
            ),
            an_acceptance(
                into_the_project_as("author"),
                json!({ "kind": "system", "id": "account_bo" }),
            ),
            Message::opening(
                RoutingKey::parse(ACCEPTED).expect("a declared key"),
                json!({ "something": "else" }),
                OffsetDateTime::UNIX_EPOCH,
            ),
        ] {
            assert!(bench.listener.handle(&unreadable).await.is_err());
        }
        // and: nobody joined
        assert!(events_after_founding(&bench).await.is_empty());
    }

    #[tokio::test]
    async fn only_an_accepted_invitation_is_heard() {
        // given: the listener
        let listener = a_founded_project().await.listener;

        // expect: it hears an acceptance
        assert!(listener.hears(&RoutingKey::parse(ACCEPTED).expect("a declared key")));
        // and: not an issuing
        assert!(
            !listener
                .hears(&RoutingKey::parse(invitations_contract::ISSUED).expect("a declared key"))
        );
    }
}
