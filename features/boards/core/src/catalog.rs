use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;
use transacting::Transaction;

use crate::board::{Board, IdeaLink, ProjectLink};
use crate::id::BoardId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardSummary {
    pub id: BoardId,
    pub project: ProjectLink,
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("the catalog of boards could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait BoardCatalog: Send + Sync {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &BoardSummary,
    ) -> Result<(), CatalogError>;

    async fn forget(
        &self,
        transaction: &mut Transaction,
        board: &BoardId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<BoardSummary>, CatalogError>;

    async fn holds(
        &self,
        transaction: &mut Transaction,
        board: BoardId,
        ideas: &[IdeaLink],
    ) -> Result<(), CatalogError>;

    async fn boards_holding(&self, idea: &IdeaLink) -> Result<Vec<BoardId>, CatalogError>;

    async fn lay_tombstone(
        &self,
        transaction: &mut Transaction,
        idea: &IdeaLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;

    async fn held_though_tombstoned(&self, board: BoardId) -> Result<Vec<IdeaLink>, CatalogError>;

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

impl BoardSummary {
    pub fn of(id: BoardId, board: &Board) -> Self {
        Self {
            id,
            project: board.project().clone(),
        }
    }
}
