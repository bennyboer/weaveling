use std::fmt::{self, Display, Formatter};

use eventsourcing::{
    Agent, Aggregate, AggregateId, AggregateType, Event, EventMetadata, EventName, Version,
};
use thiserror::Error;

use crate::id::IdeaId;
use crate::title::IdeaTitle;

pub const KIND: AggregateType = AggregateType::of("idea");

impl From<&IdeaId> for AggregateId {
    fn from(id: &IdeaId) -> Self {
        AggregateId::from(id.to_string())
    }
}

const CAPTURED: EventName = EventName::of("CAPTURED");
const RETITLED: EventName = EventName::of("RETITLED");
const DISCARDED: EventName = EventName::of("DISCARDED");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectLink(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdeaCommand {
    Capture {
        project: ProjectLink,
        title: IdeaTitle,
    },
    Retitle(IdeaTitle),
    Discard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdeaEvent {
    Captured {
        project: ProjectLink,
        title: IdeaTitle,
    },
    Retitled(IdeaTitle),
    Discarded,
    Snapshotted {
        project: ProjectLink,
        title: IdeaTitle,
        discarded: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Idea {
    project: ProjectLink,
    title: IdeaTitle,
    discarded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdeaError {
    #[error("an idea must be captured before anything else can happen to it")]
    NotCapturedYet,
    #[error("an idea cannot be captured twice")]
    AlreadyCaptured,
    #[error("a discarded idea accepts no changes")]
    Discarded,
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

impl Idea {
    pub fn project(&self) -> &ProjectLink {
        &self.project
    }

    pub fn title(&self) -> &IdeaTitle {
        &self.title
    }

    pub fn is_discarded(&self) -> bool {
        self.discarded
    }
}

impl Event for IdeaEvent {
    fn name(&self) -> EventName {
        match self {
            Self::Captured { .. } => CAPTURED,
            Self::Retitled { .. } => RETITLED,
            Self::Discarded { .. } => DISCARDED,
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

impl Aggregate for Idea {
    type Command = IdeaCommand;
    type Event = IdeaEvent;
    type Error = IdeaError;

    const KIND: AggregateType = KIND;

    fn begin(command: IdeaCommand, _agent: &Agent) -> Result<Vec<IdeaEvent>, IdeaError> {
        match command {
            IdeaCommand::Capture { project, title } => {
                Ok(vec![IdeaEvent::Captured { project, title }])
            }
            _ => Err(IdeaError::NotCapturedYet),
        }
    }

    fn from_first(event: &IdeaEvent, _metadata: &EventMetadata) -> Option<Self> {
        match event {
            IdeaEvent::Captured { project, title } => Some(Self {
                project: project.clone(),
                title: title.clone(),
                discarded: false,
            }),
            IdeaEvent::Snapshotted {
                project,
                title,
                discarded,
            } => Some(Self {
                project: project.clone(),
                title: title.clone(),
                discarded: *discarded,
            }),
            _ => None,
        }
    }

    fn decide(&self, command: IdeaCommand, _agent: &Agent) -> Result<Vec<IdeaEvent>, IdeaError> {
        if self.discarded {
            return Err(IdeaError::Discarded);
        }

        match command {
            IdeaCommand::Capture { .. } => Err(IdeaError::AlreadyCaptured),
            IdeaCommand::Retitle(to) => {
                if to == self.title {
                    return Ok(vec![]);
                }

                Ok(vec![IdeaEvent::Retitled(to)])
            }
            IdeaCommand::Discard => Ok(vec![IdeaEvent::Discarded]),
        }
    }

    fn apply(&mut self, event: &IdeaEvent, _metadata: &EventMetadata) {
        match event {
            IdeaEvent::Captured { .. } => {}
            IdeaEvent::Retitled(to) => self.title = to.clone(),
            IdeaEvent::Discarded => self.discarded = true,
            IdeaEvent::Snapshotted {
                project,
                title,
                discarded,
            } => {
                self.project = project.clone();
                self.title = title.clone();
                self.discarded = *discarded;
            }
        }
    }

    fn snapshot(&self) -> IdeaEvent {
        IdeaEvent::Snapshotted {
            project: self.project.clone(),
            title: self.title.clone(),
            discarded: self.discarded,
        }
    }
}
