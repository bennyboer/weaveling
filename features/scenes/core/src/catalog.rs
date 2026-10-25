use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;

use crate::{ProjectLink, SceneId, SceneTitle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneSummary {
    pub id: SceneId,
    pub project: ProjectLink,
    pub title: SceneTitle,
    pub opening: String,
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("the catalog of scenes could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait SceneCatalog: Send + Sync {
    async fn remember(&self, id: SceneId, project: &ProjectLink) -> Result<(), CatalogError>;

    async fn retitle(
        &self,
        id: SceneId,
        project: &ProjectLink,
        title: &SceneTitle,
        version: u64,
    ) -> Result<(), CatalogError>;

    async fn note_opening(
        &self,
        id: SceneId,
        project: &ProjectLink,
        opening: &str,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn forget(&self, id: SceneId, at: OffsetDateTime) -> Result<(), CatalogError>;

    async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;

    async fn summary_of(&self, id: SceneId) -> Result<Option<SceneSummary>, CatalogError>;

    async fn in_project_after(
        &self,
        project: &ProjectLink,
        after: Option<SceneId>,
        at_most: usize,
    ) -> Result<Vec<SceneSummary>, CatalogError>;
}
