use std::error::Error;
use time::OffsetDateTime;
use transacting::Transaction;

use async_trait::async_trait;
use eventsourcing::Version;
use thiserror::Error;

use crate::id::IdeaId;
use crate::idea::{Idea, ProjectLink};
use crate::title::IdeaTitle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdeaSummary {
    pub id: IdeaId,
    pub version: Version,
    pub project: ProjectLink,
    pub title: IdeaTitle,
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("the catalog of ideas could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait IdeaCatalog: Send + Sync {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &IdeaSummary,
    ) -> Result<(), CatalogError>;

    async fn forget(
        &self,
        transaction: &mut Transaction,
        id: &IdeaId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<IdeaSummary>, CatalogError>;

    async fn in_project_after(
        &self,
        project: &ProjectLink,
        after: Option<IdeaId>,
        at_most: usize,
    ) -> Result<Vec<IdeaSummary>, CatalogError>;

    async fn lay_project_tombstone(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn has_project_tombstone_within(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
    ) -> Result<bool, CatalogError>;
}

impl IdeaSummary {
    pub fn of(id: IdeaId, version: Version, idea: &Idea) -> Self {
        Self {
            id,
            version,
            project: idea.project().clone(),
            title: idea.title().clone(),
        }
    }
}
