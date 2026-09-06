use serde::{Deserialize, Serialize};

pub const STARTED: &str = "outline.started";
pub const SECTION_ADDED: &str = "outline.section.added";
pub const SECTION_RETITLED: &str = "outline.section.retitled";
pub const SECTION_MOVED: &str = "outline.section.moved";
pub const SECTION_PROMOTED: &str = "outline.section.promoted";
pub const SECTION_DEMOTED: &str = "outline.section.demoted";
pub const SECTION_REMOVED: &str = "outline.section.removed";
pub const PIECE_ATTACHED: &str = "outline.piece.attached";
pub const PIECE_DETACHED: &str = "outline.piece.detached";
pub const EVERY_OUTLINE: &str = "outline.#";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PlacedSectionDTO {
    pub section: String,
    pub parent: Option<String>,
    pub title: String,
    pub pieces: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OutlineDTO {
    pub id: String,
    pub version: u64,
    pub project: String,
    pub sections: Vec<PlacedSectionDTO>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AddedSectionResponse {
    pub section: String,
    pub outline: OutlineDTO,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OpenOutlineRequest {
    pub project: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AddSectionRequest {
    pub under: Option<String>,
    pub after: Option<String>,
    #[serde(default)]
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RetitleSectionRequest {
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct MoveSectionRequest {
    pub under: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AttachPieceRequest {
    pub piece: String,
    pub section: String,
    pub after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "name")]
pub enum OutlineEventDTO {
    #[serde(rename = "STARTED")]
    Started { project: String },
    #[serde(rename = "SECTION_ADDED")]
    SectionAdded {
        section: String,
        under: Option<String>,
        after: Option<String>,
        title: String,
    },
    #[serde(rename = "SECTION_RETITLED")]
    SectionRetitled { section: String, title: String },
    #[serde(rename = "SECTION_MOVED")]
    SectionMoved {
        section: String,
        under: Option<String>,
        after: Option<String>,
    },
    #[serde(rename = "SECTION_PROMOTED")]
    SectionPromoted { section: String },
    #[serde(rename = "SECTION_DEMOTED")]
    SectionDemoted { section: String },
    #[serde(rename = "SECTION_REMOVED")]
    SectionRemoved { section: String },
    #[serde(rename = "PIECE_ATTACHED")]
    PieceAttached {
        piece: String,
        to: String,
        after: Option<String>,
    },
    #[serde(rename = "PIECE_DETACHED")]
    PieceDetached { piece: String },
}
