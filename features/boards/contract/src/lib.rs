use serde::{Deserialize, Serialize};

pub const STARTED: &str = "board.started";
pub const PIECE_PINNED: &str = "board.piece-pinned";
pub const PIECE_MOVED: &str = "board.piece-moved";
pub const PIECE_UNPINNED: &str = "board.piece-unpinned";
pub const EVERY_BOARD: &str = "board.#";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct SpotDTO {
    pub x: i64,
    pub y: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PositionedPieceDTO {
    pub piece: String,
    pub spot: SpotDTO,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct BoardDTO {
    pub id: String,
    pub version: u64,
    pub project: String,
    pub pieces: Vec<PositionedPieceDTO>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OpenBoardRequest {
    pub project: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PinPieceRequest {
    pub piece: String,
    pub spot: SpotDTO,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct MovePieceRequest {
    pub spot: SpotDTO,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "name")]
pub enum BoardEventDTO {
    #[serde(rename = "STARTED")]
    Started { project: String },
    #[serde(rename = "PIECE_PINNED")]
    PiecePinned { piece: String, at: SpotDTO },
    #[serde(rename = "PIECE_MOVED")]
    PieceMoved { piece: String, to: SpotDTO },
    #[serde(rename = "PIECE_UNPINNED")]
    PieceUnpinned { piece: String },
}
