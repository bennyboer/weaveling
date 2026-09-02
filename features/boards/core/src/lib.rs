mod board;
mod catalog;
mod id;
mod service;
mod spot;

#[cfg(test)]
mod board_tests;

pub use board::{
    Board, BoardCommand, BoardError, BoardEvent, KIND, PieceLink, PositionedPiece, ProjectLink,
};
pub use catalog::{BoardCatalog, BoardSummary, CatalogError};
pub use id::BoardId;
pub use service::{BoardService, BoardServiceError, Open};
pub use spot::Spot;
