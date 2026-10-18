#![cfg_attr(not(any(feature = "postgres", feature = "sqlite")), allow(dead_code))]

pub mod sample;
#[cfg(all(test, any(feature = "postgres", feature = "sqlite")))]
pub(crate) mod stored;
pub mod suite;

use async_trait::async_trait;

use crate::aggregate::AggregateId;
use crate::codec::Codec;
use crate::store::EventStore;
use crate::testing::sample::SampleEvent;

#[async_trait]
pub trait Workbench: Sized {
    type Store: EventStore<SampleEvent> + Send + Sync;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

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
