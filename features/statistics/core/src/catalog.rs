use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;

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
        scene: &SceneLink,
        project: &ProjectLink,
        words: u64,
        counted_at: OffsetDateTime,
    ) -> Result<Option<u64>, CatalogError>;

    async fn forget_scene(&self, scene: &SceneLink) -> Result<(), CatalogError>;

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<WordCount>, CatalogError>;
}
