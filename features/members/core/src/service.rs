use std::sync::Arc;

use clock::Clock;
use eventsourcing::{
    AccountId, Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing,
    StoreError,
};

use crate::error::MembersCoreError;
use crate::members::{Members, MembersCommand, MembersEvent, ProjectLink};

#[derive(Clone)]
pub struct MembersService {
    events: Arc<EventSourcingService<Members>>,
}

impl MembersService {
    pub fn new(store: Arc<dyn EventStore<MembersEvent>>, clock: Arc<dyn Clock>) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(store, clock)),
        }
    }

    pub async fn found(
        &self,
        project: &ProjectLink,
        owner: AccountId,
        agent: &Agent,
    ) -> Result<(), MembersCoreError> {
        match self
            .events
            .begin(
                &AggregateId::from(project),
                MembersCommand::Found { owner },
                agent,
            )
            .await
        {
            Ok(_) | Err(ServiceError::Store(StoreError::Outdated { .. })) => Ok(()),
            Err(refused) => Err(refused.into()),
        }
    }

    pub async fn carry_out(
        &self,
        project: &ProjectLink,
        command: MembersCommand,
        agent: &Agent,
    ) -> Result<(), MembersCoreError> {
        let _landed = self
            .events
            .execute(&AggregateId::from(project), command, agent)
            .await?;

        Ok(())
    }

    pub async fn latest(
        &self,
        project: &ProjectLink,
    ) -> Result<Standing<Members>, MembersCoreError> {
        Ok(self.events.latest(&AggregateId::from(project)).await?)
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use clock::FixedClock;
    use eventsourcing::{AggregateType, InMemoryEventStore, Recorded, Version};
    use time::OffsetDateTime;

    use super::*;

    fn ada() -> AccountId {
        AccountId::from("account_ada")
    }

    #[tokio::test]
    async fn a_project_s_members_are_founded_once_however_often_it_is_asked() {
        let store: Arc<InMemoryEventStore<MembersEvent>> = Arc::new(InMemoryEventStore::new());
        let members = MembersService::new(
            store.clone(),
            Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        );
        let project = ProjectLink::from("project_1");

        for owner in [ada(), AccountId::from("account_grace")] {
            members
                .found(&project, owner, &Agent::System)
                .await
                .expect("founding again is no failure");
        }

        let founded = store
            .read_from(&AggregateId::from(&project), crate::KIND, Version::of(1))
            .await
            .expect("reading should succeed");
        assert_eq!(
            founded
                .into_iter()
                .map(|recorded| recorded.event)
                .collect::<Vec<_>>(),
            vec![MembersEvent::Founded { owner: ada() }]
        );
    }

    struct DownStore;

    fn down(aggregate: &AggregateId, kind: AggregateType) -> StoreError {
        StoreError::Backend {
            aggregate: aggregate.clone(),
            kind,
            detail: "the store is down".to_owned(),
        }
    }

    #[async_trait]
    impl EventStore<MembersEvent> for DownStore {
        async fn append(
            &self,
            aggregate: &AggregateId,
            kind: AggregateType,
            _expected: Version,
            _events: &[Recorded<MembersEvent>],
        ) -> Result<(), StoreError> {
            Err(down(aggregate, kind))
        }

        async fn read_from(
            &self,
            aggregate: &AggregateId,
            kind: AggregateType,
            _from: Version,
        ) -> Result<Vec<Recorded<MembersEvent>>, StoreError> {
            Err(down(aggregate, kind))
        }

        async fn read_through(
            &self,
            aggregate: &AggregateId,
            kind: AggregateType,
            _from: Version,
            _through: Version,
        ) -> Result<Vec<Recorded<MembersEvent>>, StoreError> {
            Err(down(aggregate, kind))
        }

        async fn latest_snapshot(
            &self,
            aggregate: &AggregateId,
            kind: AggregateType,
        ) -> Result<Option<Recorded<MembersEvent>>, StoreError> {
            Err(down(aggregate, kind))
        }

        async fn snapshot_at_or_before(
            &self,
            aggregate: &AggregateId,
            kind: AggregateType,
            _version: Version,
        ) -> Result<Option<Recorded<MembersEvent>>, StoreError> {
            Err(down(aggregate, kind))
        }

        async fn prune_through(
            &self,
            aggregate: &AggregateId,
            kind: AggregateType,
            _through: Version,
        ) -> Result<(), StoreError> {
            Err(down(aggregate, kind))
        }
    }

    #[tokio::test]
    async fn a_store_that_is_down_is_a_failure_to_found() {
        let members = MembersService::new(
            Arc::new(DownStore),
            Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        );

        assert!(
            members
                .found(&ProjectLink::from("project_1"), ada(), &Agent::System)
                .await
                .is_err()
        );
    }
}
