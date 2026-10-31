#![cfg_attr(not(any(feature = "postgres", feature = "sqlite")), allow(dead_code))]

pub mod sample;
#[cfg(all(test, any(feature = "postgres", feature = "sqlite")))]
pub(crate) mod stored;
pub mod suite;

use std::ops::Deref;

use async_trait::async_trait;
use transacting::{Transaction, Transactions};

use crate::aggregate::{AggregateId, AggregateType};
use crate::codec::Codec;
use crate::event::Recorded;
use crate::store::{EventStore, StoreError};
use crate::testing::sample::SampleEvent;
use crate::version::Version;

#[async_trait]
pub trait Workbench: Sized {
    type Store: EventStore<SampleEvent> + Send + Sync;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    fn transactions(&self) -> &dyn Transactions;

    fn committing(&self) -> Committing<'_, Self::Store> {
        Committing {
            store: self.store(),
            transactions: self.transactions(),
        }
    }

    async fn waiting(&self) -> Vec<String>;

    async fn cleanup(self);
}

#[async_trait]
pub trait Tables: Workbench {
    fn reading_with(&self, codec: Codec<SampleEvent>) -> Self::Store;

    async fn close(&self);

    async fn lay_the_schema_down_again(&self);

    async fn write_unreadable_event(&self, aggregate: &AggregateId, version: u64);

    async fn push_versions_below_zero(&self, aggregate: &AggregateId);

    async fn columns(&self, aggregate: &AggregateId, version: u64) -> (String, i64, String);
}

pub struct Committing<'a, S> {
    store: &'a S,
    transactions: &'a dyn Transactions,
}

impl<S: EventStore<SampleEvent>> Committing<'_, S> {
    pub async fn append(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        expected: Version,
        events: &[Recorded<SampleEvent>],
    ) -> Result<(), StoreError> {
        let mut transaction = self.begun(aggregate, kind).await?;
        self.store
            .append(&mut transaction, aggregate, kind, expected, events)
            .await?;

        self.finished(transaction, aggregate, kind).await
    }

    pub async fn prune_through(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        through: Version,
    ) -> Result<(), StoreError> {
        let mut transaction = self.begun(aggregate, kind).await?;
        self.store
            .prune_through(&mut transaction, aggregate, kind, through)
            .await?;

        self.finished(transaction, aggregate, kind).await
    }

    pub async fn forget(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<(), StoreError> {
        let mut transaction = self.begun(aggregate, kind).await?;
        self.store.forget(&mut transaction, aggregate, kind).await?;

        self.finished(transaction, aggregate, kind).await
    }

    async fn begun(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Transaction, StoreError> {
        self.transactions
            .begin()
            .await
            .map_err(|failure| StoreError::backend(aggregate, kind, failure.to_string()))
    }

    async fn finished(
        &self,
        transaction: Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<(), StoreError> {
        transaction
            .finish()
            .await
            .map_err(|failure| StoreError::backend(aggregate, kind, failure.to_string()))
    }
}

impl<S> Deref for Committing<'_, S> {
    type Target = S;

    fn deref(&self) -> &S {
        self.store
    }
}
