use serde::{Deserialize, Serialize};

pub const FRAGMENT: &str = "prose";

pub const MORE_TO_SWEEP: &str = "scene.sweep.more";
pub const MORE_QUOTATION_MARKS_TO_RESTYLE: &str = "scene.quotation.restyle.more";
pub const IDEA_LINKED: &str = "scene.idea.linked";
pub const IDEA_UNLINKED: &str = "scene.idea.unlinked";
pub const DELETED: &str = "scene.deleted";
pub const WORDS_COUNTED: &str = "scene.words.counted";
pub const CREATED: &str = "scene.created";
pub const RETITLED: &str = "scene.retitled";
pub const OPENING_CHANGED: &str = "scene.opening.changed";
pub const ASSETS_CHANGED: &str = "scene.assets.changed";
pub const TEXT_STATE_CHANGED: &str = "state.scene-text.changed";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct IdeaLinkDTO {
    pub scene: String,
    pub idea: String,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SceneDeletedDTO {
    pub scene: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SceneCreatedDTO {
    pub scene: String,
    pub project: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SceneRetitledDTO {
    pub scene: String,
    pub project: String,
    pub title: String,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OpeningChangedDTO {
    pub scene: String,
    pub project: String,
    pub opening: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct WordsCountedDTO {
    pub scene: String,
    pub project: String,
    pub words: u64,
}

/// The assets a scene holds, by their ids, sorted and each once. Today these are the images of its figures.
/// `version` is the scene's prose version they were read at, so a late, older message can be told apart.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SceneAssetsChangedDTO {
    pub scene: String,
    pub assets: Vec<String>,
    pub version: u64,
}

/// A scene's text as a reader sees it, once it has settled.
/// `version` is the scene's prose version it was read at, so a late, older message can be told apart.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SceneTextStateDTO {
    pub scene: String,
    pub blocks: Vec<BlockDTO>,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlockDTO {
    Paragraph {
        runs: Vec<RunDTO>,
    },
    Heading {
        level: u8,
        runs: Vec<RunDTO>,
    },
    Quote {
        blocks: Vec<BlockDTO>,
    },
    List {
        ordered: bool,
        items: Vec<Vec<BlockDTO>>,
    },
    Code {
        text: String,
    },
    Rule,
    Figure {
        asset: String,
        size: String,
        caption: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunDTO {
    Text { text: String, marks: MarksDTO },
    Break,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct MarksDTO {
    pub emphasis: bool,
    pub strong: bool,
    pub code: bool,
    pub link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct MoreToSweepDTO {
    pub project: String,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct MoreQuotationMarksToRestyleDTO {
    pub project: String,
    pub quotation: String,
    pub change: u64,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct QuotationRestylingDTO {
    pub change: u64,
    pub total: u64,
    pub done: u64,
    pub finished: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CreateSceneRequest {
    pub project: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SceneDTO {
    pub id: String,
    pub version: u64,
    pub project: String,
    pub title: String,
    pub ideas: Vec<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SceneSummaryDTO {
    pub id: String,
    pub project: String,
    pub title: String,
    pub opening: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ScenePageDTO {
    pub scenes: Vec<SceneSummaryDTO>,
    pub next: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct LinkIdeaRequest {
    pub idea: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RetitleSceneRequest {
    pub title: String,
}

pub mod refused {
    pub const ID_INVALID: &str = "scene-id-invalid";
    pub const TITLE_CONTROL_CHARACTER: &str = "scene-title-control-character";
    pub const TITLE_TOO_LONG: &str = "scene-title-too-long";
    pub const NEVER_RESTYLED: &str = "quotation-never-restyled";
    pub const NOT_AN_AUTHOR: &str = "not-an-author";

    pub const EVERY: [&str; 5] = [
        ID_INVALID,
        TITLE_CONTROL_CHARACTER,
        TITLE_TOO_LONG,
        NEVER_RESTYLED,
        NOT_AN_AUTHOR,
    ];
}
