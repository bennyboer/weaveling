use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;
use transacting::Transaction;

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
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &OutlineSummary,
    ) -> Result<(), CatalogError>;

    async fn forget(
        &self,
        transaction: &mut Transaction,
        outline: &OutlineId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<OutlineSummary>, CatalogError>;

    async fn holds(
        &self,
        transaction: &mut Transaction,
        outline: OutlineId,
        attachments: &[Attachment],
    ) -> Result<(), CatalogError>;

    async fn outlines_holding(
        &self,
        attachment: &Attachment,
    ) -> Result<Vec<OutlineId>, CatalogError>;

    async fn lay_tombstone(
        &self,
        transaction: &mut Transaction,
        attachment: &Attachment,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;

    async fn held_though_tombstoned(
        &self,
        outline: OutlineId,
    ) -> Result<Vec<Attachment>, CatalogError>;

    async fn lay_project_tombstone(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn has_project_tombstone(&self, project: &ProjectLink) -> Result<bool, CatalogError>;

    async fn has_project_tombstone_within(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
    ) -> Result<bool, CatalogError>;
}

impl OutlineSummary {
    pub fn of(id: OutlineId, outline: &Outline) -> Self {
        Self {
            id,
            project: outline.project().clone(),
        }
    }
}
