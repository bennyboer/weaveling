use async_trait::async_trait;
use thiserror::Error;

use crate::{Passage, PassageId};

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("passage {0} was not found")]
    NotFound(PassageId),
    #[error("passage {0} already exists")]
    Conflict(PassageId),
    #[error("the update offered to passage {0} could not be applied")]
    Unusable(PassageId),
    #[error("the passage store failed: {0}")]
    Backend(#[source] Box<dyn std::error::Error + Send + Sync>),
}

#[async_trait]
pub trait PassageStore: Send + Sync {
    async fn create(&self, passage: &Passage) -> Result<(), StoreError>;

    async fn load(&self, id: PassageId) -> Result<Passage, StoreError>;

    async fn absorb(&self, id: PassageId, update: &[u8]) -> Result<(), StoreError>;

    async fn delete(&self, id: PassageId) -> Result<(), StoreError>;
}
