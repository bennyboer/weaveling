mod agent;
mod codec;
mod reading;
mod rows;
mod schema;
mod snapshots;
mod writing;

#[cfg(test)]
mod sample;
#[cfg(test)]
mod tests;

pub use codec::Codec;
pub use schema::migrations;

use async_trait::async_trait;
use sqlx::PgPool;

use crate::aggregate::{AggregateId, AggregateType};
use crate::event::{Event, Recorded};
use crate::store::{EventStore, StoreError};
use crate::version::Version;

pub struct PostgresEventStore<E> {
    pool: PgPool,
    codec: Codec<E>,
}

impl<E> PostgresEventStore<E> {
    pub fn new(pool: PgPool, codec: Codec<E>) -> Self {
        Self { pool, codec }
    }

    fn backend_error(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        detail: String,
    ) -> StoreError {
        StoreError::Backend {
            aggregate: aggregate.clone(),
            kind,
            detail,
        }
    }

    fn outdated(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        expected: Version,
    ) -> StoreError {
        StoreError::Outdated {
            aggregate: aggregate.clone(),
            kind,
            expected,
        }
    }
}

fn as_bigint(version: Version) -> i64 {
    i64::try_from(version.count()).unwrap_or(i64::MAX)
}

#[async_trait]
impl<E> EventStore<E> for PostgresEventStore<E>
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
