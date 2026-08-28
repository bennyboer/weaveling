use std::fmt::{self, Display, Formatter};

use eventsourcing::{
    Agent, Aggregate, AggregateId, AggregateType, Event, EventMetadata, EventName, Version,
};
use thiserror::Error;

use crate::id::PieceId;
use crate::title::PieceTitle;

pub const KIND: AggregateType = AggregateType::of("piece");

impl From<&PieceId> for AggregateId {
    fn from(id: &PieceId) -> Self {
        AggregateId::from(id.to_string())
    }
}

const CAPTURED: EventName = EventName::of("CAPTURED");
const RETITLED: EventName = EventName::of("RETITLED");
const PASSAGE_ATTACHED: EventName = EventName::of("PASSAGE_ATTACHED");
const DISCARDED: EventName = EventName::of("DISCARDED");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectLink(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PassageLink(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PieceCommand {
    Capture {
        project: ProjectLink,
        title: PieceTitle,
    },
    Retitle(PieceTitle),
    AttachPassage(PassageLink),
    Discard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PieceEvent {
    Captured {
        project: ProjectLink,
        title: PieceTitle,
    },
    Retitled(PieceTitle),
    PassageAttached {
        passage: PassageLink,
    },
    Discarded,
    Snapshotted {
        project: ProjectLink,
        title: PieceTitle,
        passage: Option<PassageLink>,
        discarded: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    project: ProjectLink,
    title: PieceTitle,
    passage: Option<PassageLink>,
    discarded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PieceError {
    #[error("a piece must be captured before anything else can happen to it")]
    NotCapturedYet,
    #[error("a piece cannot be captured twice")]
    AlreadyCaptured,
    #[error("a discarded piece accepts no changes")]
    Discarded,
    #[error("this piece already has a passage")]
    AlreadyHoldsPassage,
}

impl ProjectLink {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ProjectLink {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl From<&str> for ProjectLink {
    fn from(given: &str) -> Self {
        Self(given.to_owned())
    }
}

impl Display for ProjectLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl PassageLink {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for PassageLink {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl From<&str> for PassageLink {
    fn from(given: &str) -> Self {
        Self(given.to_owned())
    }
}

impl Display for PassageLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Piece {
    pub fn project(&self) -> &ProjectLink {
        &self.project
    }

    pub fn title(&self) -> &PieceTitle {
        &self.title
    }

    pub fn passage(&self) -> Option<&PassageLink> {
        self.passage.as_ref()
    }

    pub fn is_discarded(&self) -> bool {
        self.discarded
    }
}

impl Event for PieceEvent {
    fn name(&self) -> EventName {
        match self {
            Self::Captured { .. } => CAPTURED,
            Self::Retitled { .. } => RETITLED,
            Self::PassageAttached { .. } => PASSAGE_ATTACHED,
            Self::Discarded => DISCARDED,
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

impl Aggregate for Piece {
    type Command = PieceCommand;
    type Event = PieceEvent;
    type Error = PieceError;

    const KIND: AggregateType = KIND;

    fn begin(command: PieceCommand, _agent: &Agent) -> Result<Vec<PieceEvent>, PieceError> {
        match command {
            PieceCommand::Capture { project, title } => {
                Ok(vec![PieceEvent::Captured { project, title }])
            }
            _ => Err(PieceError::NotCapturedYet),
        }
    }

    fn born(event: &PieceEvent, _metadata: &EventMetadata) -> Option<Self> {
        match event {
            PieceEvent::Captured { project, title } => Some(Self {
                project: project.clone(),
                title: title.clone(),
                passage: None,
                discarded: false,
            }),
            PieceEvent::Snapshotted {
                project,
                title,
                passage,
                discarded,
            } => Some(Self {
                project: project.clone(),
                title: title.clone(),
                passage: passage.clone(),
                discarded: *discarded,
            }),
            _ => None,
        }
    }

    fn decide(&self, command: PieceCommand, _agent: &Agent) -> Result<Vec<PieceEvent>, PieceError> {
        if self.discarded {
            return Err(PieceError::Discarded);
        }

        match command {
            PieceCommand::Capture { .. } => Err(PieceError::AlreadyCaptured),
            PieceCommand::Retitle(to) => {
                if to == self.title {
                    return Ok(vec![]);
                }

                Ok(vec![PieceEvent::Retitled(to)])
            }
            PieceCommand::AttachPassage(passage) => {
                if self.passage.is_some() {
                    return Err(PieceError::AlreadyHoldsPassage);
                }

                Ok(vec![PieceEvent::PassageAttached { passage }])
            }
            PieceCommand::Discard => Ok(vec![PieceEvent::Discarded]),
        }
    }

    fn absorb(&mut self, event: &PieceEvent, _metadata: &EventMetadata) {
        match event {
            PieceEvent::Captured { .. } => {}
            PieceEvent::Retitled(to) => self.title = to.clone(),
            PieceEvent::PassageAttached { passage } => self.passage = Some(passage.clone()),
            PieceEvent::Discarded => self.discarded = true,
            PieceEvent::Snapshotted {
                project,
                title,
                passage,
                discarded,
            } => {
                self.project = project.clone();
                self.title = title.clone();
                self.passage = passage.clone();
                self.discarded = *discarded;
            }
        }
    }

    fn snapshot(&self) -> PieceEvent {
        PieceEvent::Snapshotted {
            project: self.project.clone(),
            title: self.title.clone(),
            passage: self.passage.clone(),
            discarded: self.discarded,
        }
    }
}
