use std::error::Error;

use async_trait::async_trait;
use eventsourcing::Version;
use thiserror::Error;

use crate::id::PieceId;
use crate::piece::{PassageLink, Piece, ProjectLink};
use crate::title::PieceTitle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PieceSummary {
    pub id: PieceId,
    pub version: Version,
    pub project: ProjectLink,
    pub title: PieceTitle,
    pub passage: Option<PassageLink>,
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("the catalog of pieces could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait PieceCatalog: Send + Sync {
    async fn remember(&self, summary: &PieceSummary) -> Result<(), CatalogError>;

    async fn forget(&self, id: &PieceId) -> Result<(), CatalogError>;

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<PieceSummary>, CatalogError>;
}

impl PieceSummary {
    pub fn of(id: PieceId, version: Version, piece: &Piece) -> Self {
        Self {
            id,
            version,
            project: piece.project().clone(),
            title: piece.title().clone(),
            passage: piece.passage().cloned(),
        }
    }
}
