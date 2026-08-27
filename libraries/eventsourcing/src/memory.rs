use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;

use crate::aggregate::{AggregateId, AggregateType};
use crate::event::Recorded;
use crate::store::{EventStore, StoreError};
use crate::version::Version;

type Streams<E> = HashMap<(AggregateId, AggregateType), Vec<Recorded<E>>>;

#[derive(Debug)]
pub struct InMemoryEventStore<E> {
    streams: RwLock<Streams<E>>,
}

impl<E> InMemoryEventStore<E> {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Streams<E>> {
        self.streams.read().expect("event store lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Streams<E>> {
        self.streams.write().expect("event store lock poisoned")
    }
}

impl<E> Default for InMemoryEventStore<E> {
    fn default() -> Self {
        Self {
            streams: RwLock::new(HashMap::new()),
        }
    }
}

fn reached<E>(stream: Option<&Vec<Recorded<E>>>) -> Version {
    stream
        .and_then(|events| events.last())
        .map(|last| last.metadata.version)
        .unwrap_or(Version::ZERO)
}

#[async_trait]
impl<E> EventStore<E> for InMemoryEventStore<E>
where
    E: Clone + Send + Sync,
{
    async fn append(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        expected: Version,
        events: &[Recorded<E>],
    ) -> Result<(), StoreError> {
        let mut streams = self.write();
        let key = (aggregate.clone(), kind);

        if reached(streams.get(&key)) != expected {
            return Err(StoreError::Outdated {
                aggregate: aggregate.clone(),
                kind,
                expected,
            });
        }

        streams
            .entry(key)
            .or_default()
            .extend(events.iter().cloned());

        Ok(())
    }

    async fn read_from(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
    ) -> Result<Vec<Recorded<E>>, StoreError> {
        self.read_through(aggregate, kind, from, Version::of(u64::MAX))
            .await
    }

    async fn read_through(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
        through: Version,
    ) -> Result<Vec<Recorded<E>>, StoreError> {
        let streams = self.read();

        Ok(streams
            .get(&(aggregate.clone(), kind))
            .map(|events| {
                events
                    .iter()
                    .filter(|entry| {
                        entry.metadata.version >= from && entry.metadata.version <= through
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn latest_snapshot(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Option<Recorded<E>>, StoreError> {
        self.snapshot_at_or_before(aggregate, kind, Version::of(u64::MAX))
            .await
    }

    async fn snapshot_at_or_before(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        version: Version,
    ) -> Result<Option<Recorded<E>>, StoreError> {
        let streams = self.read();

        Ok(streams
            .get(&(aggregate.clone(), kind))
            .and_then(|events| {
                events
                    .iter()
                    .rev()
                    .find(|entry| entry.metadata.is_snapshot && entry.metadata.version <= version)
            })
            .cloned())
    }

    async fn prune_through(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        through: Version,
    ) -> Result<(), StoreError> {
        let mut streams = self.write();

        if let Some(events) = streams.get_mut(&(aggregate.clone(), kind)) {
            events.retain(|entry| entry.metadata.version > through);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::sample::SampleEvent;

    crate::conformance_tests!(InMemoryEventStore::<SampleEvent>::new());
}
