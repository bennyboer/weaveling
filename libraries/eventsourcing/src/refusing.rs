use std::sync::Arc;

use async_trait::async_trait;
use transacting::Transaction;

use crate::aggregate::{AggregateId, AggregateType};
use crate::event::Recorded;
use crate::store::{EventStore, StoreError};
use crate::version::Version;

pub struct RefusesAppending<E>(pub Arc<dyn EventStore<E>>);

#[async_trait]
impl<E: Send + Sync> EventStore<E> for RefusesAppending<E> {
    async fn append(
        &self,
        _transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        _expected: Version,
        _events: &[Recorded<E>],
    ) -> Result<(), StoreError> {
        Err(StoreError::backend(
            aggregate,
            kind,
            "the database went away",
        ))
    }

    async fn read_from(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
    ) -> Result<Vec<Recorded<E>>, StoreError> {
        self.0.read_from(aggregate, kind, from).await
    }

    async fn read_through(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
        through: Version,
    ) -> Result<Vec<Recorded<E>>, StoreError> {
        self.0.read_through(aggregate, kind, from, through).await
    }

    async fn latest_snapshot(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Option<Recorded<E>>, StoreError> {
        self.0.latest_snapshot(aggregate, kind).await
    }

    async fn snapshot_at_or_before(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        version: Version,
    ) -> Result<Option<Recorded<E>>, StoreError> {
        self.0.snapshot_at_or_before(aggregate, kind, version).await
    }

    async fn prune_through(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        through: Version,
    ) -> Result<(), StoreError> {
        self.0
            .prune_through(transaction, aggregate, kind, through)
            .await
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<(), StoreError> {
        self.0.forget(transaction, aggregate, kind).await
    }
}
