use async_trait::async_trait;
use thiserror::Error;

use crate::aggregate::{AggregateId, AggregateType};
use crate::event::Recorded;
use crate::version::Version;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StoreError {
    #[error("{kind} {aggregate} has moved on from version {expected}")]
    Outdated {
        aggregate: AggregateId,
        kind: AggregateType,
        expected: Version,
    },
    #[error("the backend holding {kind} {aggregate} failed: {detail}")]
    Backend {
        aggregate: AggregateId,
        kind: AggregateType,
        detail: String,
    },
}

#[async_trait]
pub trait EventStore<E>: Send + Sync {
    async fn append(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        expected: Version,
        events: &[Recorded<E>],
    ) -> Result<(), StoreError>;

    async fn read_from(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
    ) -> Result<Vec<Recorded<E>>, StoreError>;

    async fn read_through(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
        through: Version,
    ) -> Result<Vec<Recorded<E>>, StoreError>;

    async fn latest_snapshot(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Option<Recorded<E>>, StoreError>;

    async fn snapshot_at_or_before(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        version: Version,
    ) -> Result<Option<Recorded<E>>, StoreError>;

    async fn prune_through(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        through: Version,
    ) -> Result<(), StoreError>;
}
