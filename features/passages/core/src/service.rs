use std::sync::Arc;

use clock::Clock;
use ids::InvalidId;
use thiserror::Error;

use crate::{
    InvalidPassageTitle, Passage, PassageId, PassageStore, PassageTitle, ProjectLink, StoreError,
};

#[derive(Debug, Error)]
pub enum PassageServiceError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    Untitled(#[from] InvalidPassageTitle),
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

    pub async fn create(&self, project: &str) -> Result<Passage, PassageServiceError> {
        let passage = Passage::empty(
            PassageId::generate(self.clock.now()),
            ProjectLink::from(project),
        );

        self.store.create(&passage).await?;

        Ok(passage)
    }

    pub async fn open(&self, id: &str) -> Result<Passage, PassageServiceError> {
        let id: PassageId = id.parse()?;

        Ok(self.store.load(id).await?)
    }

    pub async fn apply(&self, id: &str, update: &[u8]) -> Result<(), PassageServiceError> {
        let id: PassageId = id.parse()?;

        Ok(self.store.apply(id, update).await?)
    }

    pub async fn in_project(
        &self,
        project: &str,
        after: Option<&str>,
        at_most: usize,
    ) -> Result<Vec<PassageId>, PassageServiceError> {
        let after = after.map(str::parse).transpose()?;

        Ok(self
            .store
            .in_project(&ProjectLink::from(project), after, at_most)
            .await?)
    }

    pub async fn retitle(&self, id: &str, title: &str) -> Result<Passage, PassageServiceError> {
        let id: PassageId = id.parse()?;
        let title = PassageTitle::new(title)?;

        self.store.retitle(id, &title).await?;

        Ok(self.store.load(id).await?)
    }

    pub async fn delete(&self, id: &str) -> Result<(), PassageServiceError> {
        let id: PassageId = id.parse()?;

        Ok(self.store.delete(id).await?)
    }
}
