use std::fmt::{self, Display, Formatter};

use eventsourcing::{
    AccountId, Agent, Aggregate, AggregateId, AggregateType, Event, EventMetadata, EventName,
    Version,
};
use thiserror::Error;
use time::OffsetDateTime;

use crate::id::NoticeId;

pub const KIND: AggregateType = AggregateType::of("notice");

impl From<&NoticeId> for AggregateId {
    fn from(id: &NoticeId) -> Self {
        AggregateId::from(id.to_string())
    }
}

const NOTED: EventName = EventName::of("NOTED");
const DISMISSED: EventName = EventName::of("DISMISSED");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectLink(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Owner,
    Author,
    Reader,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum About {
    RoleChanged {
        project: ProjectLink,
        role: Role,
        by: Option<AccountId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeCommand {
    Note { account: AccountId, about: About },
    Dismiss,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeEvent {
    Noted {
        account: AccountId,
        about: About,
    },
    Dismissed,
    Snapshotted {
        account: AccountId,
        about: About,
        noted_at: OffsetDateTime,
        dismissed_at: Option<OffsetDateTime>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    account: AccountId,
    about: About,
    noted_at: OffsetDateTime,
    dismissed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NoticeError {
    #[error("a notice must be noted before it can be dismissed")]
    NotNotedYet,
    #[error("a notice is noted once")]
    AlreadyNoted,
}

impl ProjectLink {
    pub fn as_str(&self) -> &str {
        &self.0
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

impl About {
    pub fn project(&self) -> Option<&ProjectLink> {
        match self {
            Self::RoleChanged { project, .. } => Some(project),
        }
    }

    pub fn accounts(&self) -> Vec<&AccountId> {
        match self {
            Self::RoleChanged { by, .. } => by.iter().collect(),
        }
    }
}

impl Notice {
    pub fn account(&self) -> &AccountId {
        &self.account
    }

    pub fn about(&self) -> &About {
        &self.about
    }

    pub fn noted_at(&self) -> OffsetDateTime {
        self.noted_at
    }

    pub fn is_dismissed(&self) -> bool {
        self.dismissed_at.is_some()
    }

    pub fn dismissed_at(&self) -> Option<OffsetDateTime> {
        self.dismissed_at
    }

    pub fn is_for(&self, agent: &Agent) -> bool {
        matches!(agent, Agent::Account(account) if *account == self.account)
    }
}

impl Event for NoticeEvent {
    fn name(&self) -> EventName {
        match self {
            Self::Noted { .. } => NOTED,
            Self::Dismissed => DISMISSED,
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

impl Aggregate for Notice {
    type Command = NoticeCommand;
    type Event = NoticeEvent;
    type Error = NoticeError;

    const KIND: AggregateType = KIND;

    fn begin(command: NoticeCommand, _agent: &Agent) -> Result<Vec<NoticeEvent>, NoticeError> {
        match command {
            NoticeCommand::Note { account, about } => {
                Ok(vec![NoticeEvent::Noted { account, about }])
            }
            NoticeCommand::Dismiss => Err(NoticeError::NotNotedYet),
        }
    }

    fn from_first(event: &NoticeEvent, metadata: &EventMetadata) -> Option<Self> {
        match event {
            NoticeEvent::Noted { account, about } => Some(Self {
                account: account.clone(),
                about: about.clone(),
                noted_at: metadata.occurred_at,
                dismissed_at: None,
            }),
            NoticeEvent::Snapshotted {
                account,
                about,
                noted_at,
                dismissed_at,
            } => Some(Self {
                account: account.clone(),
                about: about.clone(),
                noted_at: *noted_at,
                dismissed_at: *dismissed_at,
            }),
            NoticeEvent::Dismissed => None,
        }
    }

    fn decide(
        &self,
        command: NoticeCommand,
        _agent: &Agent,
    ) -> Result<Vec<NoticeEvent>, NoticeError> {
        match command {
            NoticeCommand::Note { .. } => Err(NoticeError::AlreadyNoted),
            NoticeCommand::Dismiss if self.is_dismissed() => Ok(Vec::new()),
            NoticeCommand::Dismiss => Ok(vec![NoticeEvent::Dismissed]),
        }
    }

    fn apply(&mut self, event: &NoticeEvent, metadata: &EventMetadata) {
        match event {
            NoticeEvent::Noted { .. } => {}
            NoticeEvent::Dismissed => self.dismissed_at = Some(metadata.occurred_at),
            NoticeEvent::Snapshotted {
                account,
                about,
                noted_at,
                dismissed_at,
            } => {
                self.account = account.clone();
                self.about = about.clone();
                self.noted_at = *noted_at;
                self.dismissed_at = *dismissed_at;
            }
        }
    }

    fn snapshot(&self) -> NoticeEvent {
        NoticeEvent::Snapshotted {
            account: self.account.clone(),
            about: self.about.clone(),
            noted_at: self.noted_at,
            dismissed_at: self.dismissed_at,
        }
    }
}
