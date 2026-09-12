use std::sync::Arc;

use async_trait::async_trait;

use crate::aggregate::{AggregateId, AggregateType};
use crate::event::{Event, Recorded};
use crate::publish::EventPublisher;
use crate::store::{EventStore, StoreError};
use crate::version::Version;

pub struct PublishingEventStore<E> {
    inner: Arc<dyn EventStore<E>>,
    publishing: Arc<dyn EventPublisher<E>>,
}

impl<E> PublishingEventStore<E> {
    pub fn wrapping(
        inner: Arc<dyn EventStore<E>>,
        publishing: Arc<dyn EventPublisher<E>>,
    ) -> Arc<Self> {
        Arc::new(Self { inner, publishing })
    }
}

#[async_trait]
impl<E> EventStore<E> for PublishingEventStore<E>
where
    E: Event + Send + Sync,
{
    async fn append(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        expected: Version,
        events: &[Recorded<E>],
    ) -> Result<(), StoreError> {
        self.inner.append(aggregate, kind, expected, events).await?;

        for happened in events {
            if !happened.event.is_publishable() {
                continue;
            }

            self.publishing
                .publish(happened)
                .await
                .map_err(|why| StoreError::Backend {
                    aggregate: aggregate.clone(),
                    kind,
                    detail: format!(
                        "version {} could not be announced: {why}",
                        happened.metadata.version
                    ),
                })?;
        }

        Ok(())
    }

    async fn read_from(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
    ) -> Result<Vec<Recorded<E>>, StoreError> {
        self.inner.read_from(aggregate, kind, from).await
    }

    async fn read_through(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
        through: Version,
    ) -> Result<Vec<Recorded<E>>, StoreError> {
        self.inner
            .read_through(aggregate, kind, from, through)
            .await
    }

    async fn latest_snapshot(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Option<Recorded<E>>, StoreError> {
        self.inner.latest_snapshot(aggregate, kind).await
    }

    async fn snapshot_at_or_before(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        version: Version,
    ) -> Result<Option<Recorded<E>>, StoreError> {
        self.inner
            .snapshot_at_or_before(aggregate, kind, version)
            .await
    }

    async fn prune_through(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        through: Version,
    ) -> Result<(), StoreError> {
        self.inner.prune_through(aggregate, kind, through).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use clock::FixedClock;
    use time::OffsetDateTime;

    use super::*;
    use crate::agent::{Agent, AgentId};
    use crate::aggregate::AggregateId;
    use crate::memory::InMemoryEventStore;
    use crate::publish::PublishError;
    use crate::service::{EventSourcingService, ServiceError};
    use crate::testing::sample::{Sample, SampleCommand, SampleEvent, SampleKind};

    fn an_author() -> Agent {
        Agent::User(AgentId::from("author-7"))
    }

    fn retitled(to: &str) -> SampleCommand {
        SampleCommand::UpdateTitle(to.to_owned())
    }

    fn a_creation() -> SampleCommand {
        SampleCommand::Create {
            title: "The Loom".to_owned(),
            description: "A silent machine.".to_owned(),
        }
    }

    #[derive(Default)]
    struct Overheard {
        published: std::sync::Mutex<Vec<Recorded<SampleEvent>>>,
    }

    impl Overheard {
        fn what_it_heard(&self) -> Vec<SampleEvent> {
            self.published
                .lock()
                .expect("published lock poisoned")
                .iter()
                .map(|entry| entry.event.clone())
                .collect()
        }

        fn snapshots(&self) -> usize {
            self.published
                .lock()
                .expect("published lock poisoned")
                .iter()
                .filter(|entry| entry.metadata.is_snapshot)
                .count()
        }
    }

    #[async_trait::async_trait]
    impl crate::publish::EventPublisher<SampleEvent> for Overheard {
        async fn publish(&self, happened: &Recorded<SampleEvent>) -> Result<(), PublishError> {
            self.published
                .lock()
                .expect("published lock poisoned")
                .push(happened.clone());

            Ok(())
        }
    }

    #[derive(Debug, thiserror::Error)]
    #[error("this publisher always refuses")]
    struct Refused;

    struct Refusing;

    #[async_trait::async_trait]
    impl crate::publish::EventPublisher<SampleEvent> for Refusing {
        async fn publish(&self, _happened: &Recorded<SampleEvent>) -> Result<(), PublishError> {
            Err(PublishError::because(Refused))
        }
    }

    fn publishing_to(
        overheard: Arc<dyn crate::publish::EventPublisher<SampleEvent>>,
    ) -> EventSourcingService<Sample> {
        EventSourcingService::new(
            PublishingEventStore::wrapping(Arc::new(InMemoryEventStore::new()), overheard),
            Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        )
    }

    #[tokio::test]
    async fn everything_appended_is_published() {
        let overheard = Arc::new(Overheard::default());
        let service = publishing_to(overheard.clone());
        let id = AggregateId::from("sample_1");

        service
            .begin(&id, a_creation(), &an_author())
            .await
            .expect("creating should succeed");
        service
            .execute(
                &id,
                SampleCommand::Rewrite {
                    title: "The Silent Loom".to_owned(),
                    description: "It remembers.".to_owned(),
                },
                &an_author(),
            )
            .await
            .expect("rewriting should succeed");

        assert_eq!(
            overheard.what_it_heard(),
            vec![
                SampleEvent::Created {
                    title: "The Loom".to_owned(),
                    description: "A silent machine.".to_owned(),
                    kind: SampleKind::Ordinary,
                },
                SampleEvent::TitleUpdated("The Silent Loom".to_owned()),
                SampleEvent::DescriptionUpdated("It remembers.".to_owned()),
            ],
            "a feature must not be able to forget to publish, so the store it is given does it"
        );
    }

    #[tokio::test]
    async fn a_command_that_changes_nothing_publishes_nothing() {
        let overheard = Arc::new(Overheard::default());
        let service = publishing_to(overheard.clone());
        let id = AggregateId::from("sample_1");
        service
            .begin(&id, a_creation(), &an_author())
            .await
            .expect("creating should succeed");

        service
            .execute(&id, a_creation(), &an_author())
            .await
            .expect_err("creating twice is refused");

        assert_eq!(
            overheard.what_it_heard().len(),
            1,
            "a refused command appends nothing, so it announces nothing"
        );
    }

    #[tokio::test]
    async fn collapsing_the_log_is_not_published() {
        let overheard = Arc::new(Overheard::default());
        let service = publishing_to(overheard.clone());
        let id = AggregateId::from("sample_1");
        service
            .begin(&id, a_creation(), &an_author())
            .await
            .expect("creating should succeed");

        for counted in 2..=100 {
            service
                .execute(&id, retitled(&format!("Title {counted}")), &an_author())
                .await
                .expect("updating should succeed");
        }

        assert_eq!(
            overheard.what_it_heard().len(),
            100,
            "one message per command, and the snapshot is not one"
        );
        assert_eq!(
            overheard.snapshots(),
            0,
            "collapsing the log is our own housekeeping and no subscriber's business"
        );
    }

    #[tokio::test]
    async fn an_event_that_declines_to_be_published_is_kept_to_ourselves() {
        let overheard = Arc::new(Overheard::default());
        let service = publishing_to(overheard.clone());
        let id = AggregateId::from("sample_1");
        service
            .begin(&id, a_creation(), &an_author())
            .await
            .expect("creating should succeed");

        service
            .execute(
                &id,
                SampleCommand::Correct("Quietly fixed".to_owned()),
                &an_author(),
            )
            .await
            .expect("correcting should succeed");

        assert_eq!(
            service.latest(&id).await.expect("exists").state.title,
            "Quietly fixed",
            "it still happened and it still shaped the aggregate"
        );
        assert_eq!(
            overheard.what_it_heard().len(),
            1,
            "an event may declare itself internal, and then nobody outside hears of it"
        );
    }

    #[tokio::test]
    async fn a_command_fails_if_what_happened_cannot_be_announced() {
        let service = publishing_to(Arc::new(Refusing));
        let id = AggregateId::from("sample_1");

        let refused = service
            .begin(&id, a_creation(), &an_author())
            .await
            .expect_err("a publisher that refuses must not be shrugged off");

        assert!(
            matches!(refused, ServiceError::Store(StoreError::Backend { .. })),
            "announcing is part of what the store was asked to do, so failing it fails the append: {refused:?}"
        );
    }

    #[tokio::test]
    async fn an_event_that_was_announced_is_still_in_the_store() {
        let overheard = Arc::new(Overheard::default());
        let service = publishing_to(overheard.clone());
        let id = AggregateId::from("sample_1");

        service
            .begin(&id, a_creation(), &an_author())
            .await
            .expect("creating should succeed");

        let standing = service.latest(&id).await.expect("reading should succeed");

        assert_eq!(standing.version, Version::of(1));
        assert_eq!(overheard.what_it_heard().len(), 1);
    }
}
