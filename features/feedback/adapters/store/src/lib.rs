use clock::text;
use eventsourcing::{AccountId, Agent, Codec};
use feedback_core::{
    About, BlockPath, Comment, CommentBlock, CommentRun, FeedbackEvent, ProjectLink,
    ReadingCopyLink, ReadingCopyPlace, SceneLink, SectionLink,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// A piece of feedback's event as the event store keeps it.
/// Its form is the store's own, apart from the aggregate's and the contract's, so it changes only together with what reads it back.
#[derive(Serialize, Deserialize)]
enum StoredFeedbackEvent {
    Drafted {
        project: String,
        about: StoredAbout,
        comment: Vec<StoredBlock>,
    },
    Reworded {
        comment: Vec<StoredBlock>,
    },
    Submitted,
    MarkedDone,
    MarkedNotDone,
    Deleted,
    /// Once the piece is deleted, it holds neither what it was about nor what it said.
    Snapshotted {
        project: String,
        about: Option<StoredAbout>,
        comment: Option<Vec<StoredBlock>>,
        written_by: StoredAgent,
        written_at: String,
        submitted_at: Option<String>,
        edited: bool,
        done: bool,
        deleted: bool,
    },
}

#[derive(Serialize, Deserialize)]
enum StoredAbout {
    ReadingCopy {
        reading_copy: String,
        place: StoredPlace,
    },
}

#[derive(Serialize, Deserialize)]
enum StoredPlace {
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
enum StoredBlock {
    Paragraph(Vec<StoredRun>),
    List {
        ordered: bool,
        items: Vec<Vec<StoredRun>>,
    },
    Quote(Vec<StoredRun>),
}

#[derive(Serialize, Deserialize)]
enum StoredRun {
    Text {
        text: String,
        emphasis: bool,
        strong: bool,
        link: Option<String>,
    },
    Break,
}

#[derive(Serialize, Deserialize)]
enum StoredAgent {
    System,
    Local,
    Account(String),
}

pub fn codec() -> Codec<FeedbackEvent> {
    Codec {
        body: |event| {
            serde_json::to_value(StoredFeedbackEvent::from(event))
                .expect("a stored feedback event is plain data and cannot fail to serialize")
        },
        event: |body| {
            serde_json::from_value::<StoredFeedbackEvent>(body)
                .ok()
                .and_then(|stored| FeedbackEvent::try_from(stored).ok())
        },
    }
}

/// Something the store holds that no longer reads back as an event.
#[derive(Debug)]
struct Unreadable;

fn moment(written: &str) -> Result<OffsetDateTime, Unreadable> {
    text::read(written).ok_or(Unreadable)
}

fn maybe_moment(written: Option<&str>) -> Result<Option<OffsetDateTime>, Unreadable> {
    written.map(moment).transpose()
}

impl From<&About> for StoredAbout {
    fn from(about: &About) -> Self {
        match about {
            About::ReadingCopy {
                reading_copy,
                place,
            } => Self::ReadingCopy {
                reading_copy: reading_copy.to_string(),
                place: StoredPlace::from(place),
            },
        }
    }
}

impl From<StoredAbout> for About {
    fn from(stored: StoredAbout) -> Self {
        match stored {
            StoredAbout::ReadingCopy {
                reading_copy,
                place,
            } => Self::ReadingCopy {
                reading_copy: ReadingCopyLink::from(reading_copy),
                place: ReadingCopyPlace::from(place),
            },
        }
    }
}

impl From<&ReadingCopyPlace> for StoredPlace {
    fn from(place: &ReadingCopyPlace) -> Self {
        match place {
            ReadingCopyPlace::Passage {
                scene,
                block_path,
                from,
                to,
                quote,
            } => Self::Passage {
                scene: scene.to_string(),
                block_path: block_path.indices().to_vec(),
                from: *from,
                to: *to,
                quote: quote.clone(),
            },
            ReadingCopyPlace::Scene(scene) => Self::Scene(scene.to_string()),
            ReadingCopyPlace::Section(section) => Self::Section(section.to_string()),
            ReadingCopyPlace::Whole => Self::Whole,
        }
    }
}

impl From<StoredPlace> for ReadingCopyPlace {
    fn from(stored: StoredPlace) -> Self {
        match stored {
            StoredPlace::Passage {
                scene,
                block_path,
                from,
                to,
                quote,
            } => Self::Passage {
                scene: SceneLink::from(scene),
                block_path: BlockPath::new(block_path),
                from,
                to,
                quote,
            },
            StoredPlace::Scene(scene) => Self::Scene(SceneLink::from(scene)),
            StoredPlace::Section(section) => Self::Section(SectionLink::from(section)),
            StoredPlace::Whole => Self::Whole,
        }
    }
}

fn stored_comment(comment: &Comment) -> Vec<StoredBlock> {
    comment.blocks().iter().map(StoredBlock::from).collect()
}

/// A comment kept, read back under the rules a comment holds to now; one that broke them no longer reads back.
fn comment_of(stored: Vec<StoredBlock>) -> Result<Comment, Unreadable> {
    Comment::new(stored.into_iter().map(CommentBlock::from).collect()).map_err(|_| Unreadable)
}

impl From<&CommentBlock> for StoredBlock {
    fn from(block: &CommentBlock) -> Self {
        match block {
            CommentBlock::Paragraph(runs) => {
                Self::Paragraph(runs.iter().map(StoredRun::from).collect())
            }
            CommentBlock::List { ordered, items } => Self::List {
                ordered: *ordered,
                items: items
                    .iter()
                    .map(|item| item.iter().map(StoredRun::from).collect())
                    .collect(),
            },
            CommentBlock::Quote(runs) => Self::Quote(runs.iter().map(StoredRun::from).collect()),
        }
    }
}

impl From<StoredBlock> for CommentBlock {
    fn from(stored: StoredBlock) -> Self {
        match stored {
            StoredBlock::Paragraph(runs) => {
                Self::Paragraph(runs.into_iter().map(CommentRun::from).collect())
            }
            StoredBlock::List { ordered, items } => Self::List {
                ordered,
                items: items
                    .into_iter()
                    .map(|item| item.into_iter().map(CommentRun::from).collect())
                    .collect(),
            },
            StoredBlock::Quote(runs) => {
                Self::Quote(runs.into_iter().map(CommentRun::from).collect())
            }
        }
    }
}

impl From<&CommentRun> for StoredRun {
    fn from(run: &CommentRun) -> Self {
        match run {
            CommentRun::Text {
                text,
                emphasis,
                strong,
                link,
            } => Self::Text {
                text: text.clone(),
                emphasis: *emphasis,
                strong: *strong,
                link: link.clone(),
            },
            CommentRun::Break => Self::Break,
        }
    }
}

impl From<StoredRun> for CommentRun {
    fn from(stored: StoredRun) -> Self {
        match stored {
            StoredRun::Text {
                text,
                emphasis,
                strong,
                link,
            } => Self::Text {
                text,
                emphasis,
                strong,
                link,
            },
            StoredRun::Break => Self::Break,
        }
    }
}

impl From<&Agent> for StoredAgent {
    fn from(agent: &Agent) -> Self {
        match agent {
            Agent::System => Self::System,
            Agent::Local => Self::Local,
            Agent::Account(account) => Self::Account(account.to_string()),
        }
    }
}

impl From<StoredAgent> for Agent {
    fn from(stored: StoredAgent) -> Self {
        match stored {
            StoredAgent::System => Self::System,
            StoredAgent::Local => Self::Local,
            StoredAgent::Account(account) => Self::Account(AccountId::from(account)),
        }
    }
}

impl From<&FeedbackEvent> for StoredFeedbackEvent {
    fn from(event: &FeedbackEvent) -> Self {
        match event {
            FeedbackEvent::Drafted {
                project,
                about,
                comment,
            } => Self::Drafted {
                project: project.to_string(),
                about: StoredAbout::from(about),
                comment: stored_comment(comment),
            },
            FeedbackEvent::Reworded(comment) => Self::Reworded {
                comment: stored_comment(comment),
            },
            FeedbackEvent::Submitted => Self::Submitted,
            FeedbackEvent::MarkedDone => Self::MarkedDone,
            FeedbackEvent::MarkedNotDone => Self::MarkedNotDone,
            FeedbackEvent::Deleted => Self::Deleted,
            FeedbackEvent::Snapshotted {
                project,
                about,
                comment,
                written_by,
                written_at,
                submitted_at,
                edited,
                done,
                deleted,
            } => Self::Snapshotted {
                project: project.to_string(),
                about: about.as_ref().map(StoredAbout::from),
                comment: comment.as_ref().map(stored_comment),
                written_by: StoredAgent::from(written_by),
                written_at: text::written(*written_at),
                submitted_at: submitted_at.map(text::written),
                edited: *edited,
                done: *done,
                deleted: *deleted,
            },
        }
    }
}

impl TryFrom<StoredFeedbackEvent> for FeedbackEvent {
    type Error = Unreadable;

    fn try_from(stored: StoredFeedbackEvent) -> Result<Self, Unreadable> {
        Ok(match stored {
            StoredFeedbackEvent::Drafted {
                project,
                about,
                comment,
            } => Self::Drafted {
                project: ProjectLink::from(project),
                about: About::from(about),
                comment: comment_of(comment)?,
            },
            StoredFeedbackEvent::Reworded { comment } => Self::Reworded(comment_of(comment)?),
            StoredFeedbackEvent::Submitted => Self::Submitted,
            StoredFeedbackEvent::MarkedDone => Self::MarkedDone,
            StoredFeedbackEvent::MarkedNotDone => Self::MarkedNotDone,
            StoredFeedbackEvent::Deleted => Self::Deleted,
            StoredFeedbackEvent::Snapshotted {
                project,
                about,
                comment,
                written_by,
                written_at,
                submitted_at,
                edited,
                done,
                deleted,
            } => Self::Snapshotted {
                project: ProjectLink::from(project),
                about: about.map(About::from),
                comment: comment.map(comment_of).transpose()?,
                written_by: Agent::from(written_by),
                written_at: moment(&written_at)?,
                submitted_at: maybe_moment(submitted_at.as_deref())?,
                edited,
                done,
                deleted,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use time::Duration;

    use super::*;

    const AT: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

    /// A comment of every kind of block and run: a paragraph with emphasis, strength, a link and a break, and a list.
    fn a_comment() -> Comment {
        let said =
            |text: &str, emphasis: bool, strong: bool, link: Option<&str>| CommentRun::Text {
                text: text.to_owned(),
                emphasis,
                strong,
                link: link.map(str::to_owned),
            };

        Comment::new(vec![
            CommentBlock::Quote(vec![
                said("Bea: ", false, true, None),
                said("Lovely.", true, false, None),
            ]),
            CommentBlock::Paragraph(vec![
                said("Which ", false, false, None),
                said("boats", true, true, Some("https://example.org/boats")),
                CommentRun::Break,
                said("?", false, false, None),
            ]),
            CommentBlock::List {
                ordered: true,
                items: vec![
                    vec![said("one", false, false, None)],
                    vec![said("two", false, true, None)],
                ],
            },
        ])
        .expect("a plain comment is fine")
    }

    fn in_the_copy(place: ReadingCopyPlace) -> About {
        About::ReadingCopy {
            reading_copy: ReadingCopyLink::from("copy_loom"),
            place,
        }
    }

    fn kept_and_read_back(event: &FeedbackEvent) -> Option<FeedbackEvent> {
        let codec = codec();

        (codec.event)((codec.body)(event))
    }

    #[test]
    fn every_event_reads_back_as_it_was_kept() {
        // given: every kind of event, drafts on every kind of place, and a snapshot by each kind of agent
        let mut every = vec![
            FeedbackEvent::Reworded(a_comment()),
            FeedbackEvent::Submitted,
            FeedbackEvent::MarkedDone,
            FeedbackEvent::MarkedNotDone,
            FeedbackEvent::Deleted,
        ];
        for place in [
            ReadingCopyPlace::Passage {
                scene: SceneLink::from("scene_harbour"),
                block_path: BlockPath::new(vec![2, 0, 1]),
                from: 4,
                to: 9,
                quote: "boats".to_owned(),
            },
            ReadingCopyPlace::Scene(SceneLink::from("scene_harbour")),
            ReadingCopyPlace::Section(SectionLink::from("section_morning")),
            ReadingCopyPlace::Whole,
        ] {
            every.push(FeedbackEvent::Drafted {
                project: ProjectLink::from("project_loom"),
                about: in_the_copy(place),
                comment: a_comment(),
            });
        }
        for (written_by, submitted_at) in [
            (Agent::System, None),
            (Agent::Local, Some(AT + Duration::minutes(5))),
            (Agent::Account(AccountId::from("account_ada")), Some(AT)),
        ] {
            every.push(FeedbackEvent::Snapshotted {
                project: ProjectLink::from("project_loom"),
                about: Some(in_the_copy(ReadingCopyPlace::Whole)),
                comment: Some(a_comment()),
                written_by,
                written_at: AT + Duration::seconds(7),
                submitted_at,
                edited: true,
                done: true,
                deleted: false,
            });
        }

        // and: a deleted piece's snapshot, which holds no words
        every.push(FeedbackEvent::Snapshotted {
            project: ProjectLink::from("project_loom"),
            about: None,
            comment: None,
            written_by: Agent::Account(AccountId::from("account_ada")),
            written_at: AT,
            submitted_at: Some(AT),
            edited: false,
            done: false,
            deleted: true,
        });

        // expect: each reads back as it was kept
        for event in every {
            assert_eq!(
                kept_and_read_back(&event).as_ref(),
                Some(&event),
                "{event:?}"
            );
        }
    }

    #[test]
    fn a_comment_that_breaks_the_rules_no_longer_reads_back() {
        // given: a kept rewording whose comment says nothing
        let kept = serde_json::json!({ "Reworded": { "comment": [{ "Paragraph": [] }] } });

        // expect: it reads back as nothing, rather than as a blank comment
        assert_eq!((codec().event)(kept), None);
    }
}
