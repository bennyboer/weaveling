use std::error::Error;

use async_trait::async_trait;
use eventsourcing::Version;
use thiserror::Error;

use crate::board::{Board, ProjectLink};
use crate::id::BoardId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardSummary {
    pub id: BoardId,
    pub version: Version,
    pub project: ProjectLink,
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("the catalog of boards could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait BoardCatalog: Send + Sync {
    async fn remember(&self, summary: &BoardSummary) -> Result<(), CatalogError>;

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<BoardSummary>, CatalogError>;
}

impl BoardSummary {
    pub fn of(id: BoardId, version: Version, board: &Board) -> Self {
        Self {
            id,
            version,
            project: board.project().clone(),
        }
    }
}
