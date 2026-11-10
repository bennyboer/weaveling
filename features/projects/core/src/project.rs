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
const TRASHED: EventName = EventName::of("TRASHED");
const RESTORED: EventName = EventName::of("RESTORED");
const DELETED: EventName = EventName::of("DELETED");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectCommand {
    Start(ProjectName, ProjectDetails),
    Rename(ProjectName),
    ChangeDetails(ProjectDetails),
    Trash { until: OffsetDateTime },
    Restore,
    Delete,
    DeleteAbandoned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectEvent {
    Started(ProjectName),
    Renamed(ProjectName),
    DetailsChanged(ProjectDetails),
    QuotationChanged(QuotationMarks),
    Trashed {
        until: OffsetDateTime,
    },
    Restored,
    Deleted,
    Snapshotted {
        name: ProjectName,
        details: ProjectDetails,
        created_at: OffsetDateTime,
        updated_at: OffsetDateTime,
        trashed_until: Option<OffsetDateTime>,
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
    trashed_until: Option<OffsetDateTime>,
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
    #[error("a project in the trash accepts no changes but being restored")]
    Trashed,
    #[error("a project not in the trash cannot be restored or purged")]
    NotTrashed,
    #[error("a project in the trash cannot be put there again")]
    AlreadyTrashed,
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

    pub fn trashed_until(&self) -> Option<OffsetDateTime> {
        self.trashed_until
    }

    pub fn is_trashed(&self) -> bool {
        self.trashed_until.is_some()
    }
}

impl Event for ProjectEvent {
    fn name(&self) -> EventName {
        match self {
            Self::Started(..) => STARTED,
            Self::Renamed(..) => RENAMED,
            Self::DetailsChanged(..) => DETAILS_CHANGED,
            Self::QuotationChanged(..) => QUOTATION_CHANGED,
            Self::Trashed { .. } => TRASHED,
            Self::Restored => RESTORED,
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
                trashed_until: None,
                deleted: false,
            }),
            ProjectEvent::Snapshotted {
                name,
                details,
                created_at,
                updated_at,
                trashed_until,
                deleted,
            } => Some(Self {
                id: metadata.aggregate.to_string(),
                name: name.clone(),
                details: details.clone(),
                created_at: *created_at,
                updated_at: *updated_at,
                trashed_until: *trashed_until,
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
            ProjectCommand::DeleteAbandoned => Ok(vec![ProjectEvent::Deleted]),
            ProjectCommand::Trash { .. } if self.is_trashed() => Err(ProjectError::AlreadyTrashed),
            ProjectCommand::Trash { until } => Ok(vec![ProjectEvent::Trashed { until }]),
            ProjectCommand::Restore | ProjectCommand::Delete if !self.is_trashed() => {
                Err(ProjectError::NotTrashed)
            }
            ProjectCommand::Restore => Ok(vec![ProjectEvent::Restored]),
            ProjectCommand::Delete => Ok(vec![ProjectEvent::Deleted]),
            _ if self.is_trashed() => Err(ProjectError::Trashed),
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
            ProjectEvent::Trashed { until } => {
                self.trashed_until = Some(*until);
                self.updated_at = metadata.occurred_at;
            }
            ProjectEvent::Restored => {
                self.trashed_until = None;
                self.updated_at = metadata.occurred_at;
            }
            ProjectEvent::Deleted => {
                self.deleted = true;
                self.updated_at = metadata.occurred_at;
            }
            ProjectEvent::Snapshotted {
                name,
                details,
                created_at,
                updated_at,
                trashed_until,
                deleted,
            } => {
                self.name = name.clone();
                self.details = details.clone();
                self.created_at = *created_at;
                self.updated_at = *updated_at;
                self.trashed_until = *trashed_until;
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
            trashed_until: self.trashed_until,
            deleted: self.deleted,
        }
    }
}
