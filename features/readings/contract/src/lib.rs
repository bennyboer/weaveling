use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ContentsDTO {
    pub project: String,
    pub title: String,
    pub parts: Vec<PartDTO>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PartDTO {
    Section {
        section: String,
        depth: u8,
        title: String,
    },
    Scene {
        scene: String,
        blocks: Vec<BlockDTO>,
    },
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
