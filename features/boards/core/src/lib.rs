mod board;
mod id;
mod spot;

#[cfg(test)]
mod board_tests;

pub use board::{
    Board, BoardCommand, BoardError, BoardEvent, KIND, PieceLink, PositionedPiece, ProjectLink,
};
pub use id::BoardId;
pub use spot::Spot;
