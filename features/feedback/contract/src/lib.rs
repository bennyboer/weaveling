use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// What a piece of feedback is about: the thing it is given on, and where in it.
/// Each kind of thing brings its own places, so a place always belongs to the kind it is in.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AboutDTO {
    ReadingCopy {
        reading_copy: String,
        place: ReadingCopyPlaceDTO,
    },
}

/// The kinds of subject feedback is given on, as `AboutDTO` and the listing name them.
pub mod kind {
    pub const READING_COPY: &str = "reading_copy";
}

/// Where in a reading copy a piece of feedback is.
/// A passage lies within one block: the scene, the path to the block in it,
/// a stretch of its text as offsets in characters, and the text itself, so the passage is shown without the copy.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReadingCopyPlaceDTO {
    Passage {
        scene: String,
        /// The indices that lead from the scene down to the block: its place among the scene's blocks,
        /// then inside a quote its place among the quote's blocks, inside a list the item's place and its place in the item.
        block_path: Vec<u32>,
        from: u32,
        to: u32,
        quote: String,
    },
    Scene {
        scene: String,
    },
    Section {
        section: String,
    },
    Whole,
}

/// What someone says in a piece of feedback: paragraphs and lists of text, in italics, in bold or linked.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommentBlockDTO {
    Paragraph {
        runs: Vec<CommentRunDTO>,
    },
    /// A list, each item one line of runs.
    List {
        ordered: bool,
        items: Vec<Vec<CommentRunDTO>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommentRunDTO {
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "is_unset")]
        emphasis: bool,
        #[serde(default, skip_serializing_if = "is_unset")]
        strong: bool,
        /// A page on the web or an address to mail.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        link: Option<String>,
    },
    Break,
}

fn is_unset(set: &bool) -> bool {
    !set
}

/// A piece of feedback, as whoever may see it finds it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeedbackDTO {
    pub id: String,
    pub about: AboutDTO,
    pub comment: Vec<CommentBlockDTO>,
    /// Whether it is still a draft, which only whoever wrote it sees.
    pub draft: bool,
    pub by_you: bool,
    /// Who wrote it, by name, where that is known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub written_by: Option<String>,
    pub written_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submitted_at: Option<String>,
    /// Whether what it says was changed after it was submitted.
    #[serde(default, skip_serializing_if = "is_unset")]
    pub edited: bool,
    /// Whether the authors marked it done.
    #[serde(default, skip_serializing_if = "is_unset")]
    pub done: bool,
}

/// How much feedback one subject holds, as the asker counts it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeedbackCountDTO {
    pub id: String,
    /// The feedback submitted and not done yet.
    pub not_done: u32,
    /// The asker's own drafts on it.
    pub your_drafts: u32,
}

/// How much feedback given is not done yet in a project, in every copy of it:
/// on each scene, on it or a passage in it, and on each section itself, by their ids. What has none is left out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct PendingDTO {
    pub scenes: BTreeMap<String, u32>,
    pub sections: BTreeMap<String, u32>,
}

/// The most subjects `GET /feedback/counts` counts at once.
pub const COUNTED_AT_ONCE: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct DraftFeedbackRequest {
    pub about: AboutDTO,
    pub comment: Vec<CommentBlockDTO>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RewriteFeedbackRequest {
    pub comment: Vec<CommentBlockDTO>,
}

pub mod refused {
    pub const COMMENT_BLANK: &str = "feedback-comment-blank";
    pub const COMMENT_CONTROL_CHARACTER: &str = "feedback-comment-control-character";
    pub const COMMENT_TOO_LONG: &str = "feedback-comment-too-long";
    pub const COMMENT_LINK_UNSAFE: &str = "feedback-comment-link-unsafe";
    pub const PASSAGE_EMPTY: &str = "feedback-passage-empty";
    pub const NOT_YOURS: &str = "feedback-not-yours";
    pub const SUBJECT_UNKNOWN: &str = "feedback-subject-unknown";
    pub const TOO_MANY_COUNTED: &str = "feedback-too-many-counted";
    pub const PLACE_UNNAMED: &str = "feedback-place-unnamed";

    pub const EVERY: [&str; 9] = [
        COMMENT_BLANK,
        COMMENT_CONTROL_CHARACTER,
        COMMENT_TOO_LONG,
        COMMENT_LINK_UNSAFE,
        PASSAGE_EMPTY,
        NOT_YOURS,
        SUBJECT_UNKNOWN,
        TOO_MANY_COUNTED,
        PLACE_UNNAMED,
    ];
}
