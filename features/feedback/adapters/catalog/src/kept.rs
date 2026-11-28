//! What a piece is about and what it says, and who wrote it, as the catalog's tables keep them, in their own form.

use eventsourcing::{AccountId, Agent};
use std::str::FromStr;

use feedback_core::{
    About, BlockPath, CatalogError, Comment, CommentBlock, CommentRun, FeedbackId, ReadingCopyLink,
    ReadingCopyPlace, SceneLink, SectionLink, Subject,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Deserialize)]
enum KeptAbout {
    ReadingCopy {
        reading_copy: String,
        place: KeptPlace,
    },
    Feedback {
        feedback: String,
    },
}

#[derive(Serialize, Deserialize)]
enum KeptPlace {
    Passage {
        scene: String,
        block_path: Vec<u32>,
        from: u32,
        to: u32,
        quote: String,
    },
    Scene(String),
    Section(String),
    Whole,
}

#[derive(Serialize, Deserialize)]
enum KeptBlock {
    Paragraph(Vec<KeptRun>),
    List {
        ordered: bool,
        items: Vec<Vec<KeptRun>>,
    },
    Quote(Vec<KeptRun>),
}

#[derive(Serialize, Deserialize)]
enum KeptRun {
    Text {
        text: String,
        emphasis: bool,
        strong: bool,
        link: Option<String>,
    },
    Break,
}

fn unreadable(failure: impl std::error::Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[derive(Debug)]
struct NoLongerReadable(&'static str);

impl std::fmt::Display for NoLongerReadable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the catalog keeps {} that no longer reads back", self.0)
    }
}

impl std::error::Error for NoLongerReadable {}

pub(crate) fn about_to_json(about: &About) -> String {
    let kept = match about {
        About::ReadingCopy {
            reading_copy,
            place,
        } => KeptAbout::ReadingCopy {
            reading_copy: reading_copy.to_string(),
            place: match place {
                ReadingCopyPlace::Passage {
                    scene,
                    block_path,
                    from,
                    to,
                    quote,
                } => KeptPlace::Passage {
                    scene: scene.to_string(),
                    block_path: block_path.indices().to_vec(),
                    from: *from,
                    to: *to,
                    quote: quote.clone(),
                },
                ReadingCopyPlace::Scene(scene) => KeptPlace::Scene(scene.to_string()),
                ReadingCopyPlace::Section(section) => KeptPlace::Section(section.to_string()),
                ReadingCopyPlace::Whole => KeptPlace::Whole,
            },
        },
        About::Feedback { feedback } => KeptAbout::Feedback {
            feedback: feedback.to_string(),
        },
    };

    serde_json::to_string(&kept).expect("what a piece is about is plain data")
}

pub(crate) fn about_from_json(json: &str) -> Result<About, CatalogError> {
    let kept: KeptAbout = serde_json::from_str(json).map_err(unreadable)?;

    Ok(match kept {
        KeptAbout::ReadingCopy {
            reading_copy,
            place,
        } => About::ReadingCopy {
            reading_copy: ReadingCopyLink::from(reading_copy),
            place: match place {
                KeptPlace::Passage {
                    scene,
                    block_path,
                    from,
                    to,
                    quote,
                } => ReadingCopyPlace::Passage {
                    scene: SceneLink::from(scene),
                    block_path: BlockPath::new(block_path),
                    from,
                    to,
                    quote,
                },
                KeptPlace::Scene(scene) => ReadingCopyPlace::Scene(SceneLink::from(scene)),
                KeptPlace::Section(section) => {
                    ReadingCopyPlace::Section(SectionLink::from(section))
                }
                KeptPlace::Whole => ReadingCopyPlace::Whole,
            },
        },
        KeptAbout::Feedback { feedback } => About::Feedback {
            feedback: FeedbackId::from_str(&feedback)
                .map_err(|_| unreadable(NoLongerReadable("the piece a reply answers")))?,
        },
    })
}

fn kept_run(run: &CommentRun) -> KeptRun {
    match run {
        CommentRun::Text {
            text,
            emphasis,
            strong,
            link,
        } => KeptRun::Text {
            text: text.clone(),
            emphasis: *emphasis,
            strong: *strong,
            link: link.clone(),
        },
        CommentRun::Break => KeptRun::Break,
    }
}

fn run_of(kept: KeptRun) -> CommentRun {
    match kept {
        KeptRun::Text {
            text,
            emphasis,
            strong,
            link,
        } => CommentRun::Text {
            text,
            emphasis,
            strong,
            link,
        },
        KeptRun::Break => CommentRun::Break,
    }
}

pub(crate) fn comment_to_json(comment: &Comment) -> String {
    let kept: Vec<KeptBlock> = comment
        .blocks()
        .iter()
        .map(|block| match block {
            CommentBlock::Paragraph(runs) => {
                KeptBlock::Paragraph(runs.iter().map(kept_run).collect())
            }
            CommentBlock::List { ordered, items } => KeptBlock::List {
                ordered: *ordered,
                items: items
                    .iter()
                    .map(|item| item.iter().map(kept_run).collect())
                    .collect(),
            },
            CommentBlock::Quote(runs) => KeptBlock::Quote(runs.iter().map(kept_run).collect()),
        })
        .collect();

    serde_json::to_string(&kept).expect("what a piece says is plain data")
}

pub(crate) fn comment_from_json(json: &str) -> Result<Comment, CatalogError> {
    let kept: Vec<KeptBlock> = serde_json::from_str(json).map_err(unreadable)?;
    let blocks = kept
        .into_iter()
        .map(|block| match block {
            KeptBlock::Paragraph(runs) => {
                CommentBlock::Paragraph(runs.into_iter().map(run_of).collect())
            }
            KeptBlock::List { ordered, items } => CommentBlock::List {
                ordered,
                items: items
                    .into_iter()
                    .map(|item| item.into_iter().map(run_of).collect())
                    .collect(),
            },
            KeptBlock::Quote(runs) => CommentBlock::Quote(runs.into_iter().map(run_of).collect()),
        })
        .collect();

    Comment::new(blocks).map_err(|_| unreadable(NoLongerReadable("a comment")))
}

/// A subject as two columns keep it: its kind and its id, named as feedback is asked for by them.
pub(crate) fn subject_columns(subject: &Subject) -> (String, String) {
    match subject {
        Subject::ReadingCopy(copy) => ("reading_copy".to_owned(), copy.to_string()),
        Subject::Feedback(piece) => ("feedback".to_owned(), piece.to_string()),
    }
}

/// Where in its subject a piece lies, as two columns keep it to be asked by: in a reading copy's text the scene,
/// on it or a passage in it, or the section itself; none for a whole copy. The exact place is kept with what it is about.
pub(crate) fn place_columns(about: &About) -> (Option<String>, Option<String>) {
    match (about.in_scene(), about.on_section()) {
        (Some(scene), _) => (Some("scene".to_owned()), Some(scene.to_string())),
        (None, Some(section)) => (Some("section".to_owned()), Some(section.to_string())),
        (None, None) => (None, None),
    }
}

/// Whoever wrote a piece, as one column keeps them: `account:<id>`, `local` or `system`.
pub(crate) fn agent_to_text(agent: &Agent) -> String {
    match agent {
        Agent::Account(account) => format!("account:{account}"),
        Agent::Local => "local".to_owned(),
        Agent::System => "system".to_owned(),
    }
}

pub(crate) fn agent_from_text(text: &str) -> Result<Agent, CatalogError> {
    match text.split_once(':') {
        Some(("account", account)) => Ok(Agent::Account(AccountId::from(account))),
        None if text == "local" => Ok(Agent::Local),
        None if text == "system" => Ok(Agent::System),
        _ => Err(unreadable(NoLongerReadable("a writer"))),
    }
}

/// A moment as the tables keep it, in microseconds since the epoch.
pub(crate) fn in_microseconds(at: OffsetDateTime) -> i64 {
    i64::try_from(at.unix_timestamp_nanos() / 1_000).expect("a moment fits a signed 64-bit column")
}

pub(crate) fn from_microseconds(micros: i64) -> Result<OffsetDateTime, CatalogError> {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(micros) * 1_000).map_err(unreadable)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(text: &str, strong: bool) -> CommentRun {
        CommentRun::Text {
            text: text.to_owned(),
            emphasis: false,
            strong,
            link: None,
        }
    }

    #[test]
    fn a_comment_is_read_back_as_it_was_kept_its_quotes_and_lists_with_it() {
        // given: a comment quoting a reply, then saying something and listing two things
        let comment = Comment::new(vec![
            CommentBlock::Quote(vec![said("Bea", true), said(": Lovely.", false)]),
            CommentBlock::Paragraph(vec![said("Thanks!", false), CommentRun::Break]),
            CommentBlock::List {
                ordered: true,
                items: vec![vec![said("one", false)], vec![said("two", true)]],
            },
        ])
        .expect("a comment saying something is fine");

        // when: it is kept and read back
        let read = comment_from_json(&comment_to_json(&comment));

        // then: it is the same comment
        assert_eq!(read.ok(), Some(comment));
    }

    #[test]
    fn a_reply_is_read_back_on_the_piece_it_replies_to() {
        // given: a reply, given on a piece
        let about = About::Feedback {
            feedback: FeedbackId::generate(time::OffsetDateTime::UNIX_EPOCH),
        };

        // when: it is kept and read back
        let read = about_from_json(&about_to_json(&about));

        // then: it is the same
        assert_eq!(read.ok(), Some(about));
    }
}
