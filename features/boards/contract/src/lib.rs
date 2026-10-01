use serde::{Deserialize, Serialize};

pub const STARTED: &str = "board.started";
pub const IDEA_PINNED: &str = "board.idea.pinned";
pub const IDEA_MOVED: &str = "board.idea.moved";
pub const IDEA_RESIZED: &str = "board.idea.resized";
pub const IDEA_RAISED: &str = "board.idea.raised";
pub const IDEA_UNPINNED: &str = "board.idea.unpinned";
pub const DISCARDED: &str = "board.discarded";
pub const EVERY_BOARD: &str = "board.#";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct SpotDTO {
    pub x: i64,
    pub y: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct SizeDTO {
    pub width: i64,
    pub height: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PositionedIdeaDTO {
    pub idea: String,
    pub spot: SpotDTO,
    pub size: SizeDTO,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct BoardDTO {
    pub id: String,
    pub version: u64,
    pub project: String,
    pub ideas: Vec<PositionedIdeaDTO>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OpenBoardRequest {
    pub project: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PinIdeaRequest {
    pub idea: String,
    pub spot: SpotDTO,
    pub size: SizeDTO,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ReshapeIdeaRequest {
    #[serde(default)]
    pub spot: Option<SpotDTO>,
    #[serde(default)]
    pub size: Option<SizeDTO>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "name")]
pub enum BoardEventDTO {
    #[serde(rename = "STARTED")]
    Started { project: String },
    #[serde(rename = "IDEA_PINNED")]
    IdeaPinned {
        idea: String,
        at: SpotDTO,
        size: SizeDTO,
    },
    #[serde(rename = "IDEA_MOVED")]
    IdeaMoved { idea: String, to: SpotDTO },
    #[serde(rename = "IDEA_RESIZED")]
    IdeaResized { idea: String, to: SizeDTO },
    #[serde(rename = "IDEA_RAISED")]
    IdeaRaised { idea: String },
    #[serde(rename = "IDEA_UNPINNED")]
    IdeaUnpinned { idea: String },
    #[serde(rename = "DISCARDED")]
    Discarded,
}
