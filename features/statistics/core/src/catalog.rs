use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;
use transacting::Transaction;

use crate::link::{ProjectLink, SceneLink};
use crate::word_count::WordCount;

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("the catalog of word counts could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait WordCountCatalog: Send + Sync {
    async fn keep(
        &self,
        transaction: &mut Transaction,
        scene: &SceneLink,
        project: &ProjectLink,
        words: u64,
        counted_at: OffsetDateTime,
    ) -> Result<Option<u64>, CatalogError>;

    async fn forget_scene(
        &self,
        transaction: &mut Transaction,
        scene: &SceneLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<WordCount>, CatalogError>;
}
