use eventsourcing::{
    Agent, Aggregate, AggregateId, AggregateType, Event, EventMetadata, EventName, Version,
};
use thiserror::Error;
use time::OffsetDateTime;

use crate::id::ProjectId;
use crate::name::ProjectName;

pub const KIND: AggregateType = AggregateType::of("project");

impl From<&ProjectId> for AggregateId {
    fn from(id: &ProjectId) -> Self {
        AggregateId::from(id.to_string())
    }
}

const STARTED: EventName = EventName::of("STARTED");
const RENAMED: EventName = EventName::of("RENAMED");
const DELETED: EventName = EventName::of("DELETED");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectCommand {
    Start(ProjectName),
    Rename(ProjectName),
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectEvent {
    Started(ProjectName),
    Renamed(ProjectName),
    Deleted,
    Snapshotted {
        name: ProjectName,
        created_at: OffsetDateTime,
        updated_at: OffsetDateTime,
        deleted: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    name: ProjectName,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProjectError {
    #[error("a project must be started before anything else can happen to it")]
    NotStartedYet,
    #[error("a project cannot be started twice")]
    AlreadyStarted,
    #[error("a deleted project accepts no changes")]
    Deleted,
}

impl Project {
    pub fn name(&self) -> &ProjectName {
        &self.name
    }

    pub fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    pub fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }

    pub fn is_deleted(&self) -> bool {
        self.deleted
    }
}

impl Event for ProjectEvent {
    fn name(&self) -> EventName {
        match self {
            Self::Started(..) => STARTED,
            Self::Renamed(..) => RENAMED,
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

impl Aggregate for Project {
    type Command = ProjectCommand;
    type Event = ProjectEvent;
    type Error = ProjectError;

    const KIND: AggregateType = KIND;

    fn begin(command: ProjectCommand, _agent: &Agent) -> Result<Vec<ProjectEvent>, ProjectError> {
        match command {
            ProjectCommand::Start(name) => Ok(vec![ProjectEvent::Started(name)]),
            _ => Err(ProjectError::NotStartedYet),
        }
    }

    fn from_first(event: &ProjectEvent, metadata: &EventMetadata) -> Option<Self> {
        match event {
            ProjectEvent::Started(name) => Some(Self {
                name: name.clone(),
                created_at: metadata.occurred_at,
                updated_at: metadata.occurred_at,
                deleted: false,
            }),
            ProjectEvent::Snapshotted {
                name,
                created_at,
                updated_at,
                deleted,
            } => Some(Self {
                name: name.clone(),
                created_at: *created_at,
                updated_at: *updated_at,
                deleted: *deleted,
            }),
            _ => None,
        }
    }

    fn decide(
        &self,
        command: ProjectCommand,
        _agent: &Agent,
    ) -> Result<Vec<ProjectEvent>, ProjectError> {
        if self.deleted {
            return Err(ProjectError::Deleted);
        }

        match command {
            ProjectCommand::Start(..) => Err(ProjectError::AlreadyStarted),
            ProjectCommand::Rename(to) => {
                if to == self.name {
                    return Ok(vec![]);
                }

                Ok(vec![ProjectEvent::Renamed(to)])
            }
            ProjectCommand::Delete => Ok(vec![ProjectEvent::Deleted]),
        }
    }

    fn apply(&mut self, event: &ProjectEvent, metadata: &EventMetadata) {
        match event {
            ProjectEvent::Started(..) => {}
            ProjectEvent::Renamed(to) => {
                self.name = to.clone();
                self.updated_at = metadata.occurred_at;
            }
            ProjectEvent::Deleted => {
                self.deleted = true;
                self.updated_at = metadata.occurred_at;
            }
            ProjectEvent::Snapshotted {
                name,
                created_at,
                updated_at,
                deleted,
            } => {
                self.name = name.clone();
                self.created_at = *created_at;
                self.updated_at = *updated_at;
                self.deleted = *deleted;
            }
        }
    }

    fn snapshot(&self) -> ProjectEvent {
        ProjectEvent::Snapshotted {
            name: self.name.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted: self.deleted,
        }
    }
}
