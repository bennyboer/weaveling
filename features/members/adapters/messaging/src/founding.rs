use async_trait::async_trait;
use inbox::{KeptHandler, Refused};
use members_core::MembersCore;
use messaging::Message;
use projects_contract::{ProjectEventDTO, STARTED};
use transacting::Transaction;

const AN_ACCOUNT: &str = "account";

pub struct FoundMembersOfStartedProjectListener {
    members: MembersCore,
}

impl FoundMembersOfStartedProjectListener {
    pub fn new(members: MembersCore) -> Self {
        Self { members }
    }
}

#[async_trait]
impl KeptHandler for FoundMembersOfStartedProjectListener {
    const NAME: &'static str = "found-members-of-started-project";
    const LISTENS_TO: &'static [&'static str] = &[STARTED];
    const WHEN_REFUSED: &'static str =
        "Whoever started a project was not made its owner, so it may stay out of their reach.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Refused> {
        let started = eventpublishing::published_in::<ProjectEventDTO>(message)?;
        let (AN_ACCOUNT, Some(owner)) = (started.agent.kind.as_str(), started.agent.id) else {
            return Ok(());
        };

        self.members
            .found(Some(transaction), &started.aggregate.id, &owner)
            .await
            .map_err(Refused::of)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use authorizing::AllowingEverything;
    use clock::FixedClock;
    use eventsourcing::{AccountId, Agent, AggregateId, EventStore, InMemoryEventStore, Version};
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
        listener: Inboxed<FoundMembersOfStartedProjectListener>,
    }

    fn bench() -> Bench {
        let store = Arc::new(InMemoryEventStore::new());

        Bench {
            listener: Inboxed::new(
                FoundMembersOfStartedProjectListener::new(MembersCore::new(
                    store.clone(),
                    Arc::new(InMemoryTransactions::new()),
                    Arc::new(InMemoryMembershipCatalog::new()),
                    Arc::new(AllowingEverything),
                    Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
                )),
                Arc::new(InMemoryTransactions::new()),
                Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(
                    OffsetDateTime::UNIX_EPOCH,
                )))),
            ),
            store,
        }
    }

    fn started_by(agent: Value) -> Message {
        Message::opening(
            RoutingKey::parse(STARTED).expect("a declared key"),
            json!({
                "event": { "version": 0, "name": "STARTED", "project_name": "The Silent Loom" },
                "aggregate": { "id": "project_1", "kind": "project", "version": 1 },
                "agent": agent,
                "occurred_at": "1970-01-01T00:00:00Z",
            }),
            OffsetDateTime::UNIX_EPOCH,
        )
    }

    async fn happened(bench: &Bench) -> Vec<MembersEvent> {
        bench
            .store
            .read_from(
                &AggregateId::from(&ProjectLink::from("project_1")),
                KIND,
                Version::of(1),
            )
            .await
            .expect("reading should succeed")
            .into_iter()
            .map(|recorded| recorded.event)
            .collect()
    }

    #[tokio::test]
    async fn whoever_started_a_project_owns_it_however_often_it_is_told() {
        let bench = bench();

        for _ in 0..2 {
            bench
                .listener
                .handle(&started_by(
                    json!({ "kind": "account", "id": "account_ada" }),
                ))
                .await
                .expect("messages come at least once");
        }

        assert_eq!(
            happened(&bench).await,
            vec![MembersEvent::Founded {
                owner: AccountId::from("account_ada")
            }]
        );
    }

    #[tokio::test]
    async fn the_founding_is_the_system_s_carrying_out_what_the_start_decided() {
        let bench = bench();

        bench
            .listener
            .handle(&started_by(
                json!({ "kind": "account", "id": "account_ada" }),
            ))
            .await
            .expect("founding should succeed");

        let recorded = bench
            .store
            .read_from(
                &AggregateId::from(&ProjectLink::from("project_1")),
                KIND,
                Version::of(1),
            )
            .await
            .expect("reading should succeed");
        assert_eq!(
            recorded
                .into_iter()
                .map(|recorded| recorded.metadata.agent)
                .collect::<Vec<_>>(),
            vec![Agent::System]
        );
    }

    #[tokio::test]
    async fn a_project_started_by_no_account_has_no_members_whatever_id_is_named() {
        let bench = bench();

        for agent in [
            json!({ "kind": "local", "id": null }),
            json!({ "kind": "system", "id": null }),
            json!({ "kind": "account", "id": null }),
            json!({ "kind": "system", "id": "account_ada" }),
        ] {
            bench
                .listener
                .handle(&started_by(agent))
                .await
                .expect("nobody to found it with is no failure");
        }

        assert!(happened(&bench).await.is_empty());
    }

    #[tokio::test]
    async fn only_a_started_project_is_heard() {
        let listener = bench().listener;

        assert!(listener.hears(&RoutingKey::parse(STARTED).expect("a declared key")));
        assert!(
            !listener
                .hears(&RoutingKey::parse(projects_contract::RENAMED).expect("a declared key"))
        );
    }

    #[tokio::test]
    async fn a_message_that_is_no_started_project_belongs_in_dead_letters() {
        let listener = bench().listener;

        assert!(
            listener
                .handle(&Message::opening(
                    RoutingKey::parse(STARTED).expect("a declared key"),
                    json!({ "something": "else" }),
                    OffsetDateTime::UNIX_EPOCH,
                ))
                .await
                .is_err()
        );
    }
}
