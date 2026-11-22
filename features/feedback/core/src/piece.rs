use eventsourcing::{
    Agent, Aggregate, AggregateContext, AggregateId, AggregateType, Event, EventMetadata,
    EventName, Version,
};
use projects_contract::context;
use thiserror::Error;
use time::OffsetDateTime;

use crate::about::About;
use crate::comment::Comment;
use crate::id::FeedbackId;
use crate::link::ProjectLink;

pub const KIND: AggregateType = AggregateType::of("feedback");

impl From<&FeedbackId> for AggregateId {
    fn from(id: &FeedbackId) -> Self {
        AggregateId::from(id.to_string())
    }
}

const DRAFTED: EventName = EventName::of("DRAFTED");
const REWORDED: EventName = EventName::of("REWORDED");
const SUBMITTED: EventName = EventName::of("SUBMITTED");
const MARKED_DONE: EventName = EventName::of("MARKED_DONE");
const MARKED_NOT_DONE: EventName = EventName::of("MARKED_NOT_DONE");
const DELETED: EventName = EventName::of("DELETED");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedbackCommand {
    Draft {
        project: ProjectLink,
        about: About,
        comment: Comment,
    },
    Reword(Comment),
    Submit,
    MarkDone,
    MarkNotDone,
    Delete(Deleter),
}

/// Who deletes a piece of feedback, as the service found the agent to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deleter {
    /// Whoever wrote it.
    Writer,
    /// An owner of its project; only once it is submitted, as someone else's draft is not there for them.
    Owner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedbackEvent {
    Drafted {
        project: ProjectLink,
        about: About,
        comment: Comment,
    },
    Reworded(Comment),
    Submitted,
    MarkedDone,
    MarkedNotDone,
    Deleted,
    Snapshotted {
        project: ProjectLink,
        about: About,
        comment: Comment,
        written_by: Agent,
        written_at: OffsetDateTime,
        submitted_at: Option<OffsetDateTime>,
        edited: bool,
        done: bool,
        deleted: bool,
    },
}

/// A piece of feedback: what someone says about something in a project, a draft only they see until they submit it.
/// Once submitted, everyone who may see its subject's feedback sees it, and the authors work it through, marking it done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feedback {
    project: ProjectLink,
    about: About,
    comment: Comment,
    written_by: Agent,
    written_at: OffsetDateTime,
    submitted_at: Option<OffsetDateTime>,
    edited: bool,
    done: bool,
    deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FeedbackRefused {
    #[error("a piece of feedback must be drafted before anything else can happen to it")]
    NotDraftedYet,
    #[error("a piece of feedback cannot be drafted twice")]
    AlreadyDrafted,
    #[error("a passage holds at least one character of one block, and quotes it")]
    PassageEmpty,
    #[error("only whoever wrote a piece of feedback may change it")]
    NotYours,
    #[error("only feedback submitted is worked through, never a draft")]
    NotSubmitted,
    #[error("a deleted piece of feedback accepts no changes")]
    Deleted,
}

impl Feedback {
    pub fn project(&self) -> &ProjectLink {
        &self.project
    }

    pub fn about(&self) -> &About {
        &self.about
    }

    pub fn comment(&self) -> &Comment {
        &self.comment
    }

    pub fn written_by(&self) -> &Agent {
        &self.written_by
    }

    pub fn written_at(&self) -> OffsetDateTime {
        self.written_at
    }

    /// When it was submitted; until then it is a draft.
    pub fn submitted_at(&self) -> Option<OffsetDateTime> {
        self.submitted_at
    }

    pub fn is_draft(&self) -> bool {
        self.submitted_at.is_none()
    }

    /// Whether what it says was changed after it was submitted.
    pub fn is_edited(&self) -> bool {
        self.edited
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    pub fn is_deleted(&self) -> bool {
        self.deleted
    }

    fn by_its_writer(&self, agent: &Agent) -> Result<(), FeedbackRefused> {
        if agent == &self.written_by {
            Ok(())
        } else {
            Err(FeedbackRefused::NotYours)
        }
    }

    fn submitted(&self) -> Result<(), FeedbackRefused> {
        if self.is_draft() {
            Err(FeedbackRefused::NotSubmitted)
        } else {
            Ok(())
        }
    }
}

impl Event for FeedbackEvent {
    fn name(&self) -> EventName {
        match self {
            Self::Drafted { .. } => DRAFTED,
            Self::Reworded(_) => REWORDED,
            Self::Submitted => SUBMITTED,
            Self::MarkedDone => MARKED_DONE,
            Self::MarkedNotDone => MARKED_NOT_DONE,
            Self::Deleted => DELETED,
            Self::Snapshotted { .. } => SNAPSHOTTED,
        }
    }

    fn version(&self) -> Version {
        Version::ZERO
    }

    fn is_snapshot(&self) -> bool {
        matches!(self, Self::Snapshotted { .. })
    }
}

impl Aggregate for Feedback {
    type Command = FeedbackCommand;
    type Event = FeedbackEvent;
    type Error = FeedbackRefused;

    const KIND: AggregateType = KIND;

    fn context(&self) -> AggregateContext {
        AggregateContext::default().with(context::PROJECT, self.project.to_string())
    }

    fn begin(
        command: FeedbackCommand,
        _agent: &Agent,
    ) -> Result<Vec<FeedbackEvent>, FeedbackRefused> {
        match command {
            FeedbackCommand::Draft {
                project,
                about,
                comment,
            } => {
                if !about.holds_something() {
                    return Err(FeedbackRefused::PassageEmpty);
                }

                Ok(vec![FeedbackEvent::Drafted {
                    project,
                    about,
                    comment,
                }])
            }
            _ => Err(FeedbackRefused::NotDraftedYet),
        }
    }

    fn from_first(event: &FeedbackEvent, metadata: &EventMetadata) -> Option<Self> {
        match event {
            FeedbackEvent::Drafted {
                project,
                about,
                comment,
            } => Some(Self {
                project: project.clone(),
                about: about.clone(),
                comment: comment.clone(),
                written_by: metadata.agent.clone(),
                written_at: metadata.occurred_at,
                submitted_at: None,
                edited: false,
                done: false,
                deleted: false,
            }),
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
            } => Some(Self {
                project: project.clone(),
                about: about.clone(),
                comment: comment.clone(),
                written_by: written_by.clone(),
                written_at: *written_at,
                submitted_at: *submitted_at,
                edited: *edited,
                done: *done,
                deleted: *deleted,
            }),
            _ => None,
        }
    }

    fn decide(
        &self,
        command: FeedbackCommand,
        agent: &Agent,
    ) -> Result<Vec<FeedbackEvent>, FeedbackRefused> {
        if self.deleted {
            return Err(FeedbackRefused::Deleted);
        }

        match command {
            FeedbackCommand::Draft { .. } => Err(FeedbackRefused::AlreadyDrafted),
            FeedbackCommand::Reword(to) => {
                self.by_its_writer(agent)?;
                if to == self.comment {
                    return Ok(vec![]);
                }

                Ok(vec![FeedbackEvent::Reworded(to)])
            }
            FeedbackCommand::Submit => {
                self.by_its_writer(agent)?;
                if !self.is_draft() {
                    return Ok(vec![]);
                }

                Ok(vec![FeedbackEvent::Submitted])
            }
            FeedbackCommand::MarkDone => {
                self.submitted()?;
                if self.done {
                    return Ok(vec![]);
                }

                Ok(vec![FeedbackEvent::MarkedDone])
            }
            FeedbackCommand::MarkNotDone => {
                self.submitted()?;
                if !self.done {
                    return Ok(vec![]);
                }

                Ok(vec![FeedbackEvent::MarkedNotDone])
            }
            FeedbackCommand::Delete(by) => {
                match by {
                    Deleter::Writer => self.by_its_writer(agent)?,
                    Deleter::Owner => self.submitted()?,
                }

                Ok(vec![FeedbackEvent::Deleted])
            }
        }
    }

    fn apply(&mut self, event: &FeedbackEvent, metadata: &EventMetadata) {
        match event {
            FeedbackEvent::Drafted { .. } => {}
            FeedbackEvent::Reworded(to) => {
                self.comment = to.clone();
                self.edited |= !self.is_draft();
            }
            FeedbackEvent::Submitted => self.submitted_at = Some(metadata.occurred_at),
            FeedbackEvent::MarkedDone => self.done = true,
            FeedbackEvent::MarkedNotDone => self.done = false,
            FeedbackEvent::Deleted => self.deleted = true,
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
            } => {
                self.project = project.clone();
                self.about = about.clone();
                self.comment = comment.clone();
                self.written_by = written_by.clone();
                self.written_at = *written_at;
                self.submitted_at = *submitted_at;
                self.edited = *edited;
                self.done = *done;
                self.deleted = *deleted;
            }
        }
    }

    fn snapshot(&self) -> FeedbackEvent {
        FeedbackEvent::Snapshotted {
            project: self.project.clone(),
            about: self.about.clone(),
            comment: self.comment.clone(),
            written_by: self.written_by.clone(),
            written_at: self.written_at,
            submitted_at: self.submitted_at,
            edited: self.edited,
            done: self.done,
            deleted: self.deleted,
        }
    }
}
