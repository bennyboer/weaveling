use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CaptureIdeaRequest {
    pub project: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RetitleIdeaRequest {
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct IdeaDTO {
    pub id: String,
    pub version: u64,
    pub project: String,
    pub title: String,
}

pub const CAPTURED: &str = "idea.captured";
pub const RETITLED: &str = "idea.retitled";
pub const DISCARDED: &str = "idea.discarded";
pub const EVERY_IDEA: &str = "idea.#";
pub const MORE_TO_SWEEP: &str = "idea.sweep.more";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct MoreToSweepDTO {
    pub project: String,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "name")]
pub enum IdeaEventDTO {
    #[serde(rename = "CAPTURED")]
    Captured { project: String, title: String },
    #[serde(rename = "RETITLED")]
    Retitled { title: String },
    #[serde(rename = "DISCARDED")]
    Discarded,
}

pub mod refused {
    pub const ID_INVALID: &str = "idea-id-invalid";
    pub const TITLE_CONTROL_CHARACTER: &str = "idea-title-control-character";
    pub const TITLE_TOO_LONG: &str = "idea-title-too-long";
    pub const DISCARDED: &str = "idea-discarded";

    pub const EVERY: [&str; 4] = [
        ID_INVALID,
        TITLE_CONTROL_CHARACTER,
        TITLE_TOO_LONG,
        DISCARDED,
    ];
}
