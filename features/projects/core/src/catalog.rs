use std::error::Error;

use async_trait::async_trait;
use eventsourcing::Version;
use thiserror::Error;
use time::OffsetDateTime;
use transacting::Transaction;

use crate::id::ProjectId;
use crate::name::ProjectName;
use crate::project::Project;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSummary {
    pub id: ProjectId,
    pub version: Version,
    pub name: ProjectName,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub trashed_until: Option<OffsetDateTime>,
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("the catalog of projects could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait ProjectCatalog: Send + Sync {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &ProjectSummary,
    ) -> Result<(), CatalogError>;

    async fn forget(
        &self,
        transaction: &mut Transaction,
        id: &ProjectId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;

    async fn all(&self) -> Result<Vec<ProjectSummary>, CatalogError>;

    async fn among(&self, ids: &[ProjectId]) -> Result<Vec<ProjectSummary>, CatalogError>;

    async fn in_the_trash(&self) -> Result<Vec<ProjectSummary>, CatalogError>;
}

impl ProjectSummary {
    pub fn of(id: ProjectId, version: Version, project: &Project) -> Self {
        Self {
            id,
            version,
            name: project.name().clone(),
            created_at: project.created_at(),
            updated_at: project.updated_at(),
            trashed_until: project.trashed_until(),
        }
    }
}
