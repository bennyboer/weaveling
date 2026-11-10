use std::sync::{Arc, Mutex};

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

    async fn read_from_within(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
    ) -> Result<Vec<Recorded<E>>, StoreError> {
        self.0
            .read_from_within(transaction, aggregate, kind, from)
            .await
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

    async fn latest_snapshot_within(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Option<Recorded<E>>, StoreError> {
        self.0
            .latest_snapshot_within(transaction, aggregate, kind)
            .await
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

    async fn forget_published(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<(), StoreError> {
        self.0.forget_published(transaction, aggregate, kind).await
    }
}

pub struct FailsAppending<E> {
    store: Arc<dyn EventStore<E>>,
    left: Mutex<u8>,
    broken: bool,
}

impl<E> FailsAppending<E> {
    pub fn losing_races(store: Arc<dyn EventStore<E>>, times: u8) -> Self {
        Self {
            store,
            left: Mutex::new(times),
            broken: false,
        }
    }

    pub fn broken(store: Arc<dyn EventStore<E>>, times: u8) -> Self {
        Self {
            store,
            left: Mutex::new(times),
            broken: true,
        }
    }

    pub fn left(&self) -> u8 {
        *self.left.lock().expect("nothing panics holding it")
    }
}

#[async_trait]
impl<E: Send + Sync> EventStore<E> for FailsAppending<E> {
    async fn append(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        expected: Version,
        events: &[Recorded<E>],
    ) -> Result<(), StoreError> {
        {
            let mut left = self.left.lock().expect("nothing panics holding it");
            if *left > 0 {
                *left -= 1;
                return Err(if self.broken {
                    StoreError::backend(aggregate, kind, "the database went away")
                } else {
                    StoreError::outdated(aggregate, kind, expected)
                });
            }
        }

        self.store
            .append(transaction, aggregate, kind, expected, events)
            .await
    }

    async fn read_from(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
    ) -> Result<Vec<Recorded<E>>, StoreError> {
        self.store.read_from(aggregate, kind, from).await
    }

    async fn read_from_within(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
    ) -> Result<Vec<Recorded<E>>, StoreError> {
        self.store
            .read_from_within(transaction, aggregate, kind, from)
            .await
    }

    async fn read_through(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
        through: Version,
    ) -> Result<Vec<Recorded<E>>, StoreError> {
        self.store
            .read_through(aggregate, kind, from, through)
            .await
    }

    async fn latest_snapshot(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Option<Recorded<E>>, StoreError> {
        self.store.latest_snapshot(aggregate, kind).await
    }

    async fn latest_snapshot_within(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Option<Recorded<E>>, StoreError> {
        self.store
            .latest_snapshot_within(transaction, aggregate, kind)
            .await
    }

    async fn snapshot_at_or_before(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        version: Version,
    ) -> Result<Option<Recorded<E>>, StoreError> {
        self.store
            .snapshot_at_or_before(aggregate, kind, version)
            .await
    }

    async fn prune_through(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        through: Version,
    ) -> Result<(), StoreError> {
        self.store
            .prune_through(transaction, aggregate, kind, through)
            .await
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<(), StoreError> {
        self.store.forget(transaction, aggregate, kind).await
    }

    async fn forget_published(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<(), StoreError> {
        self.store
            .forget_published(transaction, aggregate, kind)
            .await
    }
}
