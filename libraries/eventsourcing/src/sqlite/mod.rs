mod enqueuing;
mod instant;
mod reading;
mod schema;
mod writing;

#[cfg(test)]
mod tests;

pub use enqueuing::enqueue;
pub use schema::migrations;

use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::aggregate::{AggregateId, AggregateType};
use crate::codec::Codec;
use crate::event::{Event, Recorded};
use crate::outbox::MessageMapping;
use crate::store::{EventStore, StoreError};
use crate::stored_agent as agent;
use crate::version::Version;

pub struct SqliteEventStore<E> {
    pool: SqlitePool,
    codec: Codec<E>,
    message_for: MessageMapping<E>,
}

impl<E> SqliteEventStore<E> {
    pub fn new(pool: SqlitePool, codec: Codec<E>, message_for: MessageMapping<E>) -> Self {
        Self {
            pool,
            codec,
            message_for,
        }
    }
}

fn as_integer(version: Version) -> i64 {
    i64::try_from(version.count())
        .expect("no stream reaches nine quintillion events, so a version past i64::MAX is a bug")
}

#[async_trait]
impl<E> EventStore<E> for SqliteEventStore<E>
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
        self.write(aggregate, kind, expected, events).await
    }

    async fn read_from(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
    ) -> Result<Vec<Recorded<E>>, StoreError> {
        self.since(aggregate, kind, from).await
    }

    async fn read_through(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
        through: Version,
    ) -> Result<Vec<Recorded<E>>, StoreError> {
        self.between(aggregate, kind, from, through).await
    }

    async fn latest_snapshot(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Option<Recorded<E>>, StoreError> {
        self.newest_snapshot(aggregate, kind).await
    }

    async fn snapshot_at_or_before(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        version: Version,
    ) -> Result<Option<Recorded<E>>, StoreError> {
        self.snapshot_up_to(aggregate, kind, version).await
    }

    async fn prune_through(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        through: Version,
    ) -> Result<(), StoreError> {
        self.discard_through(aggregate, kind, through).await
    }
}
