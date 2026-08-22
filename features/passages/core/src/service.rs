use std::sync::Arc;

use clock::Clock;
use thiserror::Error;

use crate::{InvalidPassageId, Passage, PassageId, PassageStore, StoreError};

#[derive(Debug, Error)]
pub enum PassageServiceError {
    #[error(transparent)]
    InvalidId(#[from] InvalidPassageId),
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(Clone)]
pub struct PassageService {
    store: Arc<dyn PassageStore>,
    clock: Arc<dyn Clock>,
}

impl PassageService {
    pub fn new(store: Arc<dyn PassageStore>, clock: Arc<dyn Clock>) -> Self {
        Self { store, clock }
    }

    pub async fn create(&self) -> Result<Passage, PassageServiceError> {
        let passage = Passage::empty(PassageId::generate(self.clock.now()));

        self.store.create(&passage).await?;

        Ok(passage)
    }

    pub async fn open(&self, id: &str) -> Result<Passage, PassageServiceError> {
        let id: PassageId = id.parse()?;

        Ok(self.store.load(id).await?)
    }

    pub async fn absorb(&self, id: &str, update: &[u8]) -> Result<(), PassageServiceError> {
        let id: PassageId = id.parse()?;

        Ok(self.store.absorb(id, update).await?)
    }

    pub async fn delete(&self, id: &str) -> Result<(), PassageServiceError> {
        let id: PassageId = id.parse()?;

        Ok(self.store.delete(id).await?)
    }
}
