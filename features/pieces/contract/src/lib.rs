use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CapturePieceRequest {
    pub project: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RetitlePieceRequest {
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AttachPassageRequest {
    pub passage: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PieceDTO {
    pub id: String,
    pub version: u64,
    pub project: String,
    pub title: String,
    pub passage: Option<String>,
}

pub const CAPTURED: &str = "piece.captured";
pub const RETITLED: &str = "piece.retitled";
pub const PASSAGE_ATTACHED: &str = "piece.passage-attached";
pub const DISCARDED: &str = "piece.discarded";
pub const EVERY_PIECE: &str = "piece.#";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "name")]
pub enum PieceEventDTO {
    #[serde(rename = "CAPTURED")]
    Captured { project: String, title: String },
    #[serde(rename = "RETITLED")]
    Retitled { title: String },
    #[serde(rename = "PASSAGE_ATTACHED")]
    PassageAttached { passage: String },
    #[serde(rename = "DISCARDED")]
    Discarded,
}
