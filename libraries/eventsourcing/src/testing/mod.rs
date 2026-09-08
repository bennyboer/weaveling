pub mod sample;
pub mod suite;

use async_trait::async_trait;

use crate::store::EventStore;
use crate::testing::sample::SampleEvent;

#[async_trait]
pub trait Workbench: Sized {
    type Store: EventStore<SampleEvent> + Send + Sync;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    async fn cleanup(self);
}
