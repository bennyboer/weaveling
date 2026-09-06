use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;

use crate::id::OutlineId;
use crate::outline::{Outline, PieceLink, ProjectLink};

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

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<OutlineSummary>, CatalogError>;

    async fn holds(&self, outline: OutlineId, pieces: &[PieceLink]) -> Result<(), CatalogError>;

    async fn outlines_holding(&self, piece: &PieceLink) -> Result<Vec<OutlineId>, CatalogError>;
}

impl OutlineSummary {
    pub fn of(id: OutlineId, outline: &Outline) -> Self {
        Self {
            id,
            project: outline.project().clone(),
        }
    }
}
