use std::error::Error;

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
    async fn remember(&self, summary: &IdeaSummary) -> Result<(), CatalogError>;

    async fn forget(&self, id: &IdeaId) -> Result<(), CatalogError>;

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<IdeaSummary>, CatalogError>;

    async fn in_project_after(
        &self,
        project: &ProjectLink,
        after: Option<IdeaId>,
        at_most: usize,
    ) -> Result<Vec<IdeaSummary>, CatalogError>;
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
