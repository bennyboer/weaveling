use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;

use crate::id::OutlineId;
use crate::outline::{Attachment, Outline, ProjectLink};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineSummary {
    pub id: OutlineId,
    pub project: ProjectLink,
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("the catalog of outlines could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait OutlineCatalog: Send + Sync {
    async fn remember(&self, summary: &OutlineSummary) -> Result<(), CatalogError>;

    async fn forget(&self, outline: &OutlineId) -> Result<(), CatalogError>;

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<OutlineSummary>, CatalogError>;

    async fn holds(
        &self,
        outline: OutlineId,
        attachments: &[Attachment],
    ) -> Result<(), CatalogError>;

    async fn outlines_holding(
        &self,
        attachment: &Attachment,
    ) -> Result<Vec<OutlineId>, CatalogError>;

    async fn mark_gone(
        &self,
        attachment: &Attachment,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;

    async fn gone_but_held(&self, outline: OutlineId) -> Result<Vec<Attachment>, CatalogError>;
}

impl OutlineSummary {
    pub fn of(id: OutlineId, outline: &Outline) -> Self {
        Self {
            id,
            project: outline.project().clone(),
        }
    }
}
