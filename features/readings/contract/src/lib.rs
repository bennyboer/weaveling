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

/// A change to the live book, told to whoever follows it.
/// Each carries the whole of the piece that changed, so a follower swaps it in for what it holds.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BookChangeDTO {
    /// A scene's text, as it now reads.
    Text {
        scene: String,
        blocks: Vec<BlockDTO>,
    },
    /// The book's sections in reading order, with the scenes each holds.
    Outline { sections: Vec<OutlinedSectionDTO> },
    /// The book's title.
    Title { title: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OutlinedSectionDTO {
    pub section: String,
    pub depth: u8,
    pub title: String,
    pub scenes: Vec<String>,
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
