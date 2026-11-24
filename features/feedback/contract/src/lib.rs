use std::collections::BTreeMap;
use std::fmt;

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

pub const DRAFTED: &str = "feedback.drafted";
pub const REWORDED: &str = "feedback.reworded";
pub const SUBMITTED: &str = "feedback.submitted";
pub const MARKED_DONE: &str = "feedback.marked.done";
pub const MARKED_NOT_DONE: &str = "feedback.marked.not.done";
pub const DELETED: &str = "feedback.deleted";
pub const EVERY_FEEDBACK: &str = "feedback.#";

/// What happened to a piece of feedback, as other features hear it. Its project is in the message's context.
/// What it is about is told once, when it is drafted. What it says is never told: it stays the feedback feature's,
/// and only whoever may see it reads it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "name")]
pub enum FeedbackEventDTO {
    #[serde(rename = "DRAFTED")]
    Drafted { about: AboutDTO },
    #[serde(rename = "REWORDED")]
    Reworded,
    #[serde(rename = "SUBMITTED")]
    Submitted,
    #[serde(rename = "MARKED_DONE")]
    MarkedDone,
    #[serde(rename = "MARKED_NOT_DONE")]
    MarkedNotDone,
    #[serde(rename = "DELETED")]
    Deleted,
}

/// What whoever follows a project's feedback live under `feedback`, about `{ project }`, is told of a piece given there,
/// the frame being of the piece: that it changed, given, reworded or marked, or that it is gone.
/// It never says what a piece says or is about, and never tells of a draft;
/// whoever hears it asks again for the feedback they show.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GivenFeedbackChangeDTO {
    Changed,
    Gone,
}

/// Which feedback is asked for, as `GET /feedback/on`, `GET /feedback/drafts` and `POST /feedback/submitted` name it in their query:
///
/// `project=p&subjects=reading_copy:c1,reading_copy:c2&places=scene:a,scene:b,section:s&given_by=account:r`
///
/// Every parameter named narrows what the others leave: the feedback in project `p`,
/// and on one of the subjects named, and at one of the places named, and given by one of the givers named.
/// The values inside one parameter are alternatives: on `c1` or on `c2`; at `a`, at `b` or at `s`.
/// A parameter left out does not narrow, and with none named the asking is refused as `feedback-place-unnamed`.
///
/// A subject is what feedback is given on as a whole, named as its kind and its id: `reading_copy:<id>`.
/// A place is where inside a subject, named the same way: in a reading copy's text `scene:<id>`,
/// holding the feedback on the scene and on passages in it, or `section:<id>`, holding what is on the section itself.
/// So a section with everything in it is asked for as the section and its scenes: `places=section:s,scene:a,scene:b`.
/// A giver is whoever gave feedback, named as its kind and its id too: `account:<id>`,
/// or `local` where this Weaveling instance keeps no accounts; `GET /feedback/givers?project=` lists them.
/// A kind not known is refused as `feedback-subject-unknown`, `feedback-place-unknown` or `feedback-giver-unknown`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AskedDTO {
    pub project: Option<String>,
    pub subjects: Vec<SubjectDTO>,
    pub places: Vec<PlaceDTO>,
    pub given_by: Vec<GiverDTO>,
}

/// A subject named in `subjects=`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubjectDTO {
    ReadingCopy(String),
}

/// A place named in `places=`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaceDTO {
    Scene(String),
    Section(String),
}

/// A giver named in `given_by=`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GiverDTO {
    Local,
    Account(String),
}

/// Whoever gave feedback in a project, as `GET /feedback/givers?project=` lists them, in the order they first submitted any:
/// how `given_by=` names them, whether it is the asker, and their name where it is known.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ListedGiverDTO {
    pub giver: String,
    pub by_you: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A subject, place or giver whose kind is not known; it holds what was named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownKind(pub String);

impl fmt::Display for SubjectDTO {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadingCopy(id) => write!(f, "reading_copy:{id}"),
        }
    }
}

impl fmt::Display for PlaceDTO {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scene(id) => write!(f, "scene:{id}"),
            Self::Section(id) => write!(f, "section:{id}"),
        }
    }
}

impl fmt::Display for GiverDTO {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local => f.write_str("local"),
            Self::Account(id) => write!(f, "account:{id}"),
        }
    }
}

impl GiverDTO {
    /// The givers named in `given_by=`, parted by commas.
    pub fn listed(named: &str) -> Result<Vec<Self>, UnknownKind> {
        named
            .split(',')
            .filter(|one| !one.is_empty())
            .map(|one| match one.split_once(':') {
                None if one == "local" => Ok(Self::Local),
                Some(("account", id)) => Ok(Self::Account(id.to_owned())),
                _ => Err(UnknownKind(one.to_owned())),
            })
            .collect()
    }
}

impl SubjectDTO {
    /// The subjects named in `subjects=`, parted by commas.
    pub fn listed(named: &str) -> Result<Vec<Self>, UnknownKind> {
        listed(named, |kind, id| match kind {
            "reading_copy" => Some(Self::ReadingCopy(id.to_owned())),
            _ => None,
        })
    }
}

impl PlaceDTO {
    /// The places named in `places=`, parted by commas.
    pub fn listed(named: &str) -> Result<Vec<Self>, UnknownKind> {
        listed(named, |kind, id| match kind {
            "scene" => Some(Self::Scene(id.to_owned())),
            "section" => Some(Self::Section(id.to_owned())),
            _ => None,
        })
    }
}

fn listed<T>(named: &str, read: impl Fn(&str, &str) -> Option<T>) -> Result<Vec<T>, UnknownKind> {
    named
        .split(',')
        .filter(|one| !one.is_empty())
        .map(|one| {
            one.split_once(':')
                .and_then(|(kind, id)| read(kind, id))
                .ok_or_else(|| UnknownKind(one.to_owned()))
        })
        .collect()
}

impl AskedDTO {
    /// The query it is asked with, without the leading `?`; the ids are as safe in a query as they are given.
    pub fn to_query(&self) -> String {
        fn joined<T: ToString>(named: &str, listed: &[T]) -> Option<String> {
            (!listed.is_empty()).then(|| {
                let values: Vec<String> = listed.iter().map(ToString::to_string).collect();
                format!("{named}={}", values.join(","))
            })
        }

        [
            self.project
                .as_ref()
                .map(|project| format!("project={project}")),
            joined("subjects", &self.subjects),
            joined("places", &self.places),
            joined("given_by", &self.given_by),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("&")
    }

    /// Whether anything at all is named, as asking for nothing in particular is not asking.
    pub fn names_anything(&self) -> bool {
        self != &Self::default()
    }
}

/// In which order `GET /feedback/on?order=` lists feedback submitted, by when it was submitted; oldest first unless asked.
pub mod order {
    pub const OLDEST: &str = "oldest";
    pub const NEWEST: &str = "newest";
}

/// What feedback given is, as `GET /feedback/on?status=` asks for it.
pub mod status {
    pub const PENDING: &str = "pending";
    pub const DONE: &str = "done";
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
    /// All of it, on a whole copy too, which lies on no scene and no section.
    pub total: u32,
}

/// The most subjects `GET /feedback/counts` counts, and the most scenes `GET /feedback/on` looks on, at once.
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
    pub const TOO_MANY_COUNTED: &str = "feedback-too-many-counted";
    pub const PLACE_UNNAMED: &str = "feedback-place-unnamed";
    pub const STATUS_UNKNOWN: &str = "feedback-status-unknown";
    pub const ORDER_UNKNOWN: &str = "feedback-order-unknown";
    pub const SUBJECT_UNKNOWN: &str = "feedback-subject-unknown";
    pub const PLACE_UNKNOWN: &str = "feedback-place-unknown";
    pub const GIVER_UNKNOWN: &str = "feedback-giver-unknown";
    pub const READING_COPY_CLOSED: &str = "feedback-reading-copy-closed";
    /// The reading copy was made a moment ago, and the feedback has not heard of it yet; asking again shortly succeeds.
    pub const READING_COPY_UNKNOWN: &str = "feedback-reading-copy-unknown";
    pub const MARKED_BY_AUTHORS: &str = "feedback-marked-by-authors";

    pub const EVERY: [&str; 16] = [
        COMMENT_BLANK,
        COMMENT_CONTROL_CHARACTER,
        COMMENT_TOO_LONG,
        COMMENT_LINK_UNSAFE,
        PASSAGE_EMPTY,
        NOT_YOURS,
        TOO_MANY_COUNTED,
        PLACE_UNNAMED,
        STATUS_UNKNOWN,
        ORDER_UNKNOWN,
        SUBJECT_UNKNOWN,
        PLACE_UNKNOWN,
        GIVER_UNKNOWN,
        READING_COPY_CLOSED,
        READING_COPY_UNKNOWN,
        MARKED_BY_AUTHORS,
    ];
}
