use async_trait::async_trait;
use eventsourcing::{AccountId, Agent};
use members_core::{MembersService, ProjectLink};
use messaging::{Failure, Handler, Message};
use projects_contract::{ProjectEventDTO, STARTED};

const AN_ACCOUNT: &str = "account";

pub struct FoundMembersOfStartedProjectListener {
    members: MembersService,
}

impl FoundMembersOfStartedProjectListener {
    pub fn new(members: MembersService) -> Self {
        Self { members }
    }
}

#[async_trait]
impl Handler for FoundMembersOfStartedProjectListener {
    const NAME: &'static str = "found-members-of-started-project";
    const LISTENS_TO: &'static [&'static str] = &[STARTED];
    const WHEN_REFUSED: &'static str =
        "Whoever started a project was not made its owner, so it may stay out of their reach.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let started = eventpublishing::published_in::<ProjectEventDTO>(message)?;
        let (AN_ACCOUNT, Some(owner)) = (started.agent.kind.as_str(), started.agent.id) else {
            return Ok(());
        };

        Ok(self
            .members
            .found(
                &ProjectLink::from(started.aggregate.id),
                AccountId::from(owner),
                &Agent::System,
            )
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use clock::FixedClock;
    use eventsourcing::{AggregateId, EventStore, InMemoryEventStore, Version};
    use members_core::{KIND, MembersEvent};
    use messaging::{Listener, RoutingKey};
    use serde_json::{Value, json};
    use time::OffsetDateTime;

    use super::*;

    struct Bench {
        store: Arc<InMemoryEventStore<MembersEvent>>,
        listener: FoundMembersOfStartedProjectListener,
    }

    fn bench() -> Bench {
        let store = Arc::new(InMemoryEventStore::new());

        Bench {
            listener: FoundMembersOfStartedProjectListener::new(MembersService::new(
                store.clone(),
                Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
            )),
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
                .take_in(&started_by(
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
                .take_in(&started_by(agent))
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
                .take_in(&Message::opening(
                    RoutingKey::parse(STARTED).expect("a declared key"),
                    json!({ "something": "else" }),
                    OffsetDateTime::UNIX_EPOCH,
                ))
                .await
                .is_err()
        );
    }
}
