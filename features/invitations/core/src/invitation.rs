use email::EmailAddress;
use eventsourcing::{Agent, Aggregate, AggregateType, Event, EventMetadata, EventName, Version};
use thiserror::Error;
use time::{Duration, OffsetDateTime};

pub const KIND: AggregateType = AggregateType::of("invitation");

const ISSUED: EventName = EventName::of("ISSUED");
const ACCEPTED: EventName = EventName::of("ACCEPTED");
const WITHDRAWN: EventName = EventName::of("WITHDRAWN");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvitationCommand {
    Issue { to: EmailAddress },
    Accept { at: OffsetDateTime },
    Withdraw,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvitationEvent {
    Issued {
        to: EmailAddress,
    },
    Accepted,
    Withdrawn,
    Snapshotted {
        to: EmailAddress,
        issued_at: OffsetDateTime,
        outcome: Outcome,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Open,
    Accepted,
    Withdrawn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invitation {
    to: EmailAddress,
    issued_at: OffsetDateTime,
    outcome: Outcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvitationError {
    #[error("an invitation must be issued before anything else can happen to it")]
    NotIssuedYet,
    #[error("an invitation is issued once")]
    AlreadyIssued,
    #[error("this invitation was taken up already")]
    AlreadyAccepted,
    #[error("this invitation was withdrawn")]
    Withdrawn,
    #[error("this invitation has run out")]
    Expired,
}

impl Invitation {
    pub const LASTS: Duration = Duration::days(7);

    pub fn to(&self) -> &EmailAddress {
        &self.to
    }

    pub fn issued_at(&self) -> OffsetDateTime {
        self.issued_at
    }

    pub fn until(&self) -> OffsetDateTime {
        self.issued_at + Self::LASTS
    }

    pub fn outcome(&self) -> Outcome {
        self.outcome
    }

    pub fn is_expired(&self, now: OffsetDateTime) -> bool {
        now > self.until()
    }
}

impl Event for InvitationEvent {
    fn name(&self) -> EventName {
        match self {
            Self::Issued { .. } => ISSUED,
            Self::Accepted => ACCEPTED,
            Self::Withdrawn => WITHDRAWN,
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

impl Aggregate for Invitation {
    type Command = InvitationCommand;
    type Event = InvitationEvent;
    type Error = InvitationError;

    const KIND: AggregateType = KIND;

    fn begin(
        command: InvitationCommand,
        _agent: &Agent,
    ) -> Result<Vec<InvitationEvent>, InvitationError> {
        match command {
            InvitationCommand::Issue { to } => Ok(vec![InvitationEvent::Issued { to }]),
            InvitationCommand::Accept { .. } | InvitationCommand::Withdraw => {
                Err(InvitationError::NotIssuedYet)
            }
        }
    }

    fn from_first(event: &InvitationEvent, metadata: &EventMetadata) -> Option<Self> {
        match event {
            InvitationEvent::Issued { to } => Some(Self {
                to: to.clone(),
                issued_at: metadata.occurred_at,
                outcome: Outcome::Open,
            }),
            InvitationEvent::Snapshotted {
                to,
                issued_at,
                outcome,
            } => Some(Self {
                to: to.clone(),
                issued_at: *issued_at,
                outcome: *outcome,
            }),
            InvitationEvent::Accepted | InvitationEvent::Withdrawn => None,
        }
    }

    fn decide(
        &self,
        command: InvitationCommand,
        _agent: &Agent,
    ) -> Result<Vec<InvitationEvent>, InvitationError> {
        match (command, self.outcome) {
            (InvitationCommand::Issue { .. }, _) => Err(InvitationError::AlreadyIssued),
            (_, Outcome::Accepted) => Err(InvitationError::AlreadyAccepted),
            (InvitationCommand::Withdraw, Outcome::Withdrawn) => Ok(vec![]),
            (InvitationCommand::Withdraw, Outcome::Open) => Ok(vec![InvitationEvent::Withdrawn]),
            (InvitationCommand::Accept { .. }, Outcome::Withdrawn) => {
                Err(InvitationError::Withdrawn)
            }
            (InvitationCommand::Accept { at }, Outcome::Open) if self.is_expired(at) => {
                Err(InvitationError::Expired)
            }
            (InvitationCommand::Accept { .. }, Outcome::Open) => {
                Ok(vec![InvitationEvent::Accepted])
            }
        }
    }

    fn apply(&mut self, event: &InvitationEvent, _metadata: &EventMetadata) {
        match event {
            InvitationEvent::Issued { .. } => {}
            InvitationEvent::Accepted => self.outcome = Outcome::Accepted,
            InvitationEvent::Withdrawn => self.outcome = Outcome::Withdrawn,
            InvitationEvent::Snapshotted {
                to,
                issued_at,
                outcome,
            } => {
                self.to = to.clone();
                self.issued_at = *issued_at;
                self.outcome = *outcome;
            }
        }
    }

    fn snapshot(&self) -> InvitationEvent {
        InvitationEvent::Snapshotted {
            to: self.to.clone(),
            issued_at: self.issued_at,
            outcome: self.outcome,
        }
    }
}
