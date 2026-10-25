use std::iter;

use eventsourcing::{
    Agent, Aggregate, AggregateContext, AggregateId, AggregateType, Event, EventMetadata,
    EventName, Version,
};
use projects_contract::context;
use thiserror::Error;
use time::OffsetDateTime;

use crate::details::{ProjectDetails, QuotationMarks};
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
const DETAILS_CHANGED: EventName = EventName::of("DETAILS_CHANGED");
const QUOTATION_CHANGED: EventName = EventName::of("QUOTATION_CHANGED");
const DELETED: EventName = EventName::of("DELETED");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectCommand {
    Start(ProjectName, ProjectDetails),
    Rename(ProjectName),
    ChangeDetails(ProjectDetails),
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectEvent {
    Started(ProjectName),
    Renamed(ProjectName),
    DetailsChanged(ProjectDetails),
    QuotationChanged(QuotationMarks),
    Deleted,
    Snapshotted {
        name: ProjectName,
        details: ProjectDetails,
        created_at: OffsetDateTime,
        updated_at: OffsetDateTime,
        deleted: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    id: String,
    name: ProjectName,
    details: ProjectDetails,
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

    pub fn details(&self) -> &ProjectDetails {
        &self.details
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
            Self::DetailsChanged(..) => DETAILS_CHANGED,
            Self::QuotationChanged(..) => QUOTATION_CHANGED,
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

    fn context(&self) -> AggregateContext {
        AggregateContext::default().with(context::PROJECT, self.id.clone())
    }

    fn begin(command: ProjectCommand, _agent: &Agent) -> Result<Vec<ProjectEvent>, ProjectError> {
        match command {
            ProjectCommand::Start(name, details) => {
                let detailed = (details != ProjectDetails::default())
                    .then_some(ProjectEvent::DetailsChanged(details));

                Ok(iter::once(ProjectEvent::Started(name))
                    .chain(detailed)
                    .collect())
            }
            _ => Err(ProjectError::NotStartedYet),
        }
    }

    fn from_first(event: &ProjectEvent, metadata: &EventMetadata) -> Option<Self> {
        match event {
            ProjectEvent::Started(name) => Some(Self {
                id: metadata.aggregate.to_string(),
                name: name.clone(),
                details: ProjectDetails::default(),
                created_at: metadata.occurred_at,
                updated_at: metadata.occurred_at,
                deleted: false,
            }),
            ProjectEvent::Snapshotted {
                name,
                details,
                created_at,
                updated_at,
                deleted,
            } => Some(Self {
                id: metadata.aggregate.to_string(),
                name: name.clone(),
                details: details.clone(),
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
            ProjectCommand::ChangeDetails(details) => {
                if details == self.details {
                    return Ok(vec![]);
                }
                let style = details.quotation_marks();
                let restyled = (style != self.details.quotation_marks())
                    .then_some(ProjectEvent::QuotationChanged(style));

                Ok(iter::once(ProjectEvent::DetailsChanged(details))
                    .chain(restyled)
                    .collect())
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
            ProjectEvent::DetailsChanged(details) => {
                self.details = details.clone();
                self.updated_at = metadata.occurred_at;
            }
            ProjectEvent::QuotationChanged(..) => {}
            ProjectEvent::Deleted => {
                self.deleted = true;
                self.updated_at = metadata.occurred_at;
            }
            ProjectEvent::Snapshotted {
                name,
                details,
                created_at,
                updated_at,
                deleted,
            } => {
                self.name = name.clone();
                self.details = details.clone();
                self.created_at = *created_at;
                self.updated_at = *updated_at;
                self.deleted = *deleted;
            }
        }
    }

    fn snapshot(&self) -> ProjectEvent {
        ProjectEvent::Snapshotted {
            name: self.name.clone(),
            details: self.details.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted: self.deleted,
        }
    }
}
