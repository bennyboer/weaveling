use eventsourcing::{
    Agent, Aggregate, AggregateContext, AggregateId, AggregateType, Event, EventMetadata,
    EventName, Version,
};
use projects_contract::context;
use thiserror::Error;
use time::OffsetDateTime;

use crate::id::ReadingCopyId;
use crate::link::ProjectLink;
use crate::reading_copies::Extent;
use crate::reading_copy_name::ReadingCopyName;

pub const KIND: AggregateType = AggregateType::of("reading-copy");

impl From<&ReadingCopyId> for AggregateId {
    fn from(id: &ReadingCopyId) -> Self {
        AggregateId::from(id.to_string())
    }
}

const MADE: EventName = EventName::of("MADE");
const RENAMED: EventName = EventName::of("RENAMED");
const EXTENDED: EventName = EventName::of("EXTENDED");
const DELETED: EventName = EventName::of("DELETED");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

/// Until when a copy is open for reading and feedback, and when it is deleted; none for no end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Times {
    pub open_until: Option<OffsetDateTime>,
    pub deleted_on: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadingCopyCommand {
    Make {
        project: ProjectLink,
        name: ReadingCopyName,
        extent: Extent,
        times: Times,
    },
    Rename(ReadingCopyName),
    Extend(Times),
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadingCopyEvent {
    Made {
        project: ProjectLink,
        name: ReadingCopyName,
        extent: Extent,
        times: Times,
    },
    Renamed(ReadingCopyName),
    Extended(Times),
    Deleted,
    Snapshotted {
        project: ProjectLink,
        name: ReadingCopyName,
        extent: Extent,
        times: Times,
        made_by: Agent,
        made_at: OffsetDateTime,
        deleted: bool,
    },
}

/// A copy of the work, frozen when it was made, under a name, open for a while and kept for a while.
/// What it holds is kept apart, under its id, and never changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingCopy {
    project: ProjectLink,
    name: ReadingCopyName,
    extent: Extent,
    times: Times,
    made_by: Agent,
    made_at: OffsetDateTime,
    deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReadingCopyError {
    #[error("a reading copy must be made before anything else can happen to it")]
    NotMadeYet,
    #[error("a reading copy cannot be made twice")]
    AlreadyMade,
    #[error("a deleted reading copy accepts no changes")]
    Deleted,
    #[error("a reading copy cannot stay open beyond its deletion")]
    OpenBeyondDeletion,
    #[error("a reading copy's times only move later")]
    Earlier,
}

impl ReadingCopy {
    pub fn project(&self) -> &ProjectLink {
        &self.project
    }

    pub fn name(&self) -> &ReadingCopyName {
        &self.name
    }

    pub fn extent(&self) -> Extent {
        self.extent
    }

    pub fn times(&self) -> Times {
        self.times
    }

    pub fn made_by(&self) -> &Agent {
        &self.made_by
    }

    pub fn made_at(&self) -> OffsetDateTime {
        self.made_at
    }

    pub fn is_deleted(&self) -> bool {
        self.deleted
    }
}

impl Times {
    /// Whether the copy closes for good before it is deleted, or with it; never after.
    fn hold_together(self) -> bool {
        match (self.open_until, self.deleted_on) {
            (Some(open), Some(deleted)) => open <= deleted,
            (None, Some(_)) => false,
            (_, None) => true,
        }
    }

    /// Whether each time stands where it stood or later; a time with no end stays so.
    fn are_no_earlier_than(self, before: Times) -> bool {
        let no_earlier =
            |now: Option<OffsetDateTime>, then: Option<OffsetDateTime>| match (now, then) {
                (_, None) => now.is_none(),
                (None, Some(_)) => true,
                (Some(now), Some(then)) => now >= then,
            };

        no_earlier(self.open_until, before.open_until)
            && no_earlier(self.deleted_on, before.deleted_on)
    }
}

impl Event for ReadingCopyEvent {
    fn name(&self) -> EventName {
        match self {
            Self::Made { .. } => MADE,
            Self::Renamed(_) => RENAMED,
            Self::Extended(_) => EXTENDED,
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

impl Aggregate for ReadingCopy {
    type Command = ReadingCopyCommand;
    type Event = ReadingCopyEvent;
    type Error = ReadingCopyError;

    const KIND: AggregateType = KIND;

    fn context(&self) -> AggregateContext {
        AggregateContext::default().with(context::PROJECT, self.project.to_string())
    }

    fn begin(
        command: ReadingCopyCommand,
        _agent: &Agent,
    ) -> Result<Vec<ReadingCopyEvent>, ReadingCopyError> {
        match command {
            ReadingCopyCommand::Make {
                project,
                name,
                extent,
                times,
            } => {
                if !times.hold_together() {
                    return Err(ReadingCopyError::OpenBeyondDeletion);
                }

                Ok(vec![ReadingCopyEvent::Made {
                    project,
                    name,
                    extent,
                    times,
                }])
            }
            _ => Err(ReadingCopyError::NotMadeYet),
        }
    }

    fn from_first(event: &ReadingCopyEvent, metadata: &EventMetadata) -> Option<Self> {
        match event {
            ReadingCopyEvent::Made {
                project,
                name,
                extent,
                times,
            } => Some(Self {
                project: project.clone(),
                name: name.clone(),
                extent: *extent,
                times: *times,
                made_by: metadata.agent.clone(),
                made_at: metadata.occurred_at,
                deleted: false,
            }),
            ReadingCopyEvent::Snapshotted {
                project,
                name,
                extent,
                times,
                made_by,
                made_at,
                deleted,
            } => Some(Self {
                project: project.clone(),
                name: name.clone(),
                extent: *extent,
                times: *times,
                made_by: made_by.clone(),
                made_at: *made_at,
                deleted: *deleted,
            }),
            _ => None,
        }
    }

    fn decide(
        &self,
        command: ReadingCopyCommand,
        _agent: &Agent,
    ) -> Result<Vec<ReadingCopyEvent>, ReadingCopyError> {
        if self.deleted {
            return Err(ReadingCopyError::Deleted);
        }

        match command {
            ReadingCopyCommand::Make { .. } => Err(ReadingCopyError::AlreadyMade),
            ReadingCopyCommand::Rename(to) => {
                if to == self.name {
                    return Ok(vec![]);
                }

                Ok(vec![ReadingCopyEvent::Renamed(to)])
            }
            ReadingCopyCommand::Extend(to) => {
                if !to.are_no_earlier_than(self.times) {
                    return Err(ReadingCopyError::Earlier);
                }
                if !to.hold_together() {
                    return Err(ReadingCopyError::OpenBeyondDeletion);
                }
                if to == self.times {
                    return Ok(vec![]);
                }

                Ok(vec![ReadingCopyEvent::Extended(to)])
            }
            ReadingCopyCommand::Delete => Ok(vec![ReadingCopyEvent::Deleted]),
        }
    }

    fn apply(&mut self, event: &ReadingCopyEvent, _metadata: &EventMetadata) {
        match event {
            ReadingCopyEvent::Made { .. } => {}
            ReadingCopyEvent::Renamed(to) => self.name = to.clone(),
            ReadingCopyEvent::Extended(to) => self.times = *to,
            ReadingCopyEvent::Deleted => self.deleted = true,
            ReadingCopyEvent::Snapshotted {
                project,
                name,
                extent,
                times,
                made_by,
                made_at,
                deleted,
            } => {
                self.project = project.clone();
                self.name = name.clone();
                self.extent = *extent;
                self.times = *times;
                self.made_by = made_by.clone();
                self.made_at = *made_at;
                self.deleted = *deleted;
            }
        }
    }

    fn snapshot(&self) -> ReadingCopyEvent {
        ReadingCopyEvent::Snapshotted {
            project: self.project.clone(),
            name: self.name.clone(),
            extent: self.extent,
            times: self.times,
            made_by: self.made_by.clone(),
            made_at: self.made_at,
            deleted: self.deleted,
        }
    }
}
