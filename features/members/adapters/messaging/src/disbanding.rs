use async_trait::async_trait;
use members_core::MembersCore;
use messaging::{Failure, Handler, Message};
use projects_contract::{DELETED, ProjectEventDTO};

pub struct DisbandMembersOfDeletedProjectListener {
    members: MembersCore,
}

impl DisbandMembersOfDeletedProjectListener {
    pub fn new(members: MembersCore) -> Self {
        Self { members }
    }
}

#[async_trait]
impl Handler for DisbandMembersOfDeletedProjectListener {
    const NAME: &'static str = "disband-members-of-deleted-project";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str =
        "The members of a deleted project were not let go, so its owners may still invite into it.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let deleted = eventpublishing::published_in::<ProjectEventDTO>(message)?;

        Ok(self.members.disband(&deleted.aggregate.id).await?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use authorizing::AllowingEverything;
    use clock::FixedClock;
    use eventsourcing::{Agent, AggregateId, EventStore, InMemoryEventStore, Version};
    use members_catalog::InMemoryMembershipCatalog;
    use members_core::{KIND, MembersEvent, ProjectLink, Role};
    use messaging::{Listener, RoutingKey};
    use serde_json::json;
    use time::OffsetDateTime;

    use super::*;

    struct Bench {
        store: Arc<InMemoryEventStore<MembersEvent>>,
        members: MembersCore,
        listener: DisbandMembersOfDeletedProjectListener,
    }

    fn bench() -> Bench {
        let store = Arc::new(InMemoryEventStore::new());
        let members = MembersCore::new(
            store.clone(),
            Arc::new(InMemoryMembershipCatalog::new()),
            Arc::new(AllowingEverything),
            Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        );

        Bench {
            listener: DisbandMembersOfDeletedProjectListener::new(members.clone()),
            members,
            store,
        }
    }

    fn deleted() -> Message {
        Message::opening(
            RoutingKey::parse(DELETED).expect("a declared key"),
            json!({
                "event": { "version": 0, "name": "DELETED" },
                "aggregate": { "id": "project_1", "kind": "project", "version": 4 },
                "agent": { "kind": "account", "id": "account_ada" },
                "occurred_at": "1970-01-01T00:00:00Z",
            }),
            OffsetDateTime::UNIX_EPOCH,
        )
    }

    async fn happened(bench: &Bench) -> Vec<(MembersEvent, Agent)> {
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
            .map(|recorded| (recorded.event, recorded.metadata.agent))
            .collect()
    }

    #[tokio::test]
    async fn a_deleted_project_s_members_are_disbanded_by_the_system_once_however_often_it_is_told()
    {
        let bench = bench();
        assert!(
            bench
                .listener
                .hears(&RoutingKey::parse(DELETED).expect("a declared key"))
        );
        bench
            .members
            .found("project_1", "account_ada")
            .await
            .expect("founding should succeed");
        bench
            .members
            .join("project_1", "account_bo", Role::Reader)
            .await
            .expect("joining should succeed");

        for _ in 0..2 {
            bench
                .listener
                .take_in(&deleted())
                .await
                .expect("messages come at least once");
        }

        let told = happened(&bench).await;
        assert_eq!(told.len(), 3);
        assert_eq!(told[2], (MembersEvent::Disbanded, Agent::System));
    }

    #[tokio::test]
    async fn a_deletion_heard_before_the_founding_leaves_nothing_to_found_and_nobody_to_join() {
        let bench = bench();

        bench
            .listener
            .take_in(&deleted())
            .await
            .expect("members never founded are disbanded all the same");
        bench
            .members
            .found("project_1", "account_ada")
            .await
            .expect("a founding heard late is no failure");
        bench
            .members
            .join("project_1", "account_bo", Role::Reader)
            .await
            .expect("an invitation taken up into a deleted project is no failure either");

        assert_eq!(
            happened(&bench).await,
            vec![(MembersEvent::Disbanded, Agent::System)]
        );
    }
}
