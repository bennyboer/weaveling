use email::EmailAddress;
use eventsourcing::{Agent, Aggregate, AggregateType, Event, EventMetadata, EventName, Version};
use thiserror::Error;
use time::{Duration, OffsetDateTime};

use crate::scope::Scope;
use crate::token::InvitationTokenDigest;

pub const KIND: AggregateType = AggregateType::of("invitation");

const ISSUED: EventName = EventName::of("ISSUED");
const ACCEPTED: EventName = EventName::of("ACCEPTED");
const WITHDRAWN: EventName = EventName::of("WITHDRAWN");
const DECLINED: EventName = EventName::of("DECLINED");
const RETIRED: EventName = EventName::of("RETIRED");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvitationCommand {
    Issue {
        to: EmailAddress,
        digest: InvitationTokenDigest,
        until: OffsetDateTime,
        scope: Scope,
        by_an_operator: bool,
    },
    Accept {
        at: OffsetDateTime,
    },
    Withdraw,
    Decline {
        reason: DeclineReason,
    },
    Retire,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvitationEvent {
    Issued {
        to: EmailAddress,
        digest: InvitationTokenDigest,
        until: OffsetDateTime,
        scope: Scope,
        by_an_operator: bool,
    },
    Accepted,
    Withdrawn,
    Declined {
        reason: DeclineReason,
    },
    Retired,
    Snapshotted {
        to: EmailAddress,
        scope: Scope,
        by_an_operator: bool,
        issued_at: OffsetDateTime,
        until: OffsetDateTime,
        outcome: Outcome,
        settled_at: Option<OffsetDateTime>,
        retired: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Open,
    Accepted,
    Withdrawn,
    Declined(DeclineReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclineReason {
    AddressHasAccount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invitation {
    to: EmailAddress,
    scope: Scope,
    by_an_operator: bool,
    issued_at: OffsetDateTime,
    until: OffsetDateTime,
    outcome: Outcome,
    settled_at: Option<OffsetDateTime>,
    retired: bool,
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
    #[error("this invitation was declined, its address having an account already")]
    Declined,
}

impl Invitation {
    pub const LASTS: Duration = Duration::days(7);

    pub fn to(&self) -> &EmailAddress {
        &self.to
    }

    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    pub fn is_by_an_operator(&self) -> bool {
        self.by_an_operator
    }

    pub fn issued_at(&self) -> OffsetDateTime {
        self.issued_at
    }

    pub fn settled_at(&self) -> Option<OffsetDateTime> {
        self.settled_at
    }

    pub fn is_retired(&self) -> bool {
        self.retired
    }

    fn settle(&mut self, outcome: Outcome, metadata: &EventMetadata) {
        self.outcome = outcome;
        self.settled_at = Some(metadata.occurred_at);
    }

    pub fn until(&self) -> OffsetDateTime {
        self.until
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
            Self::Declined { .. } => DECLINED,
            Self::Retired => RETIRED,
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
            InvitationCommand::Issue {
                to,
                digest,
                until,
                scope,
                by_an_operator,
            } => Ok(vec![InvitationEvent::Issued {
                to,
                digest,
                until,
                scope,
                by_an_operator,
            }]),
            InvitationCommand::Accept { .. }
            | InvitationCommand::Withdraw
            | InvitationCommand::Decline { .. }
            | InvitationCommand::Retire => Err(InvitationError::NotIssuedYet),
        }
    }

    fn from_first(event: &InvitationEvent, metadata: &EventMetadata) -> Option<Self> {
        match event {
            InvitationEvent::Issued {
                to,
                until,
                scope,
                by_an_operator,
                ..
            } => Some(Self {
                to: to.clone(),
                scope: scope.clone(),
                by_an_operator: *by_an_operator,
                issued_at: metadata.occurred_at,
                until: *until,
                outcome: Outcome::Open,
                settled_at: None,
                retired: false,
            }),
            InvitationEvent::Snapshotted {
                to,
                scope,
                by_an_operator,
                issued_at,
                until,
                outcome,
                settled_at,
                retired,
            } => Some(Self {
                to: to.clone(),
                scope: scope.clone(),
                by_an_operator: *by_an_operator,
                issued_at: *issued_at,
                until: *until,
                outcome: *outcome,
                settled_at: *settled_at,
                retired: *retired,
            }),
            InvitationEvent::Accepted
            | InvitationEvent::Withdrawn
            | InvitationEvent::Declined { .. }
            | InvitationEvent::Retired => None,
        }
    }

    fn decide(
        &self,
        command: InvitationCommand,
        _agent: &Agent,
    ) -> Result<Vec<InvitationEvent>, InvitationError> {
        match (command, self.outcome) {
            (InvitationCommand::Issue { .. }, _) => Err(InvitationError::AlreadyIssued),
            (InvitationCommand::Retire, _) if self.retired => Ok(vec![]),
            (InvitationCommand::Retire, _) => Ok(vec![InvitationEvent::Retired]),
            (_, Outcome::Accepted) => Err(InvitationError::AlreadyAccepted),
            (InvitationCommand::Withdraw, Outcome::Withdrawn | Outcome::Declined(_)) => Ok(vec![]),
            (InvitationCommand::Decline { .. }, Outcome::Declined(_)) => Ok(vec![]),
            (InvitationCommand::Decline { .. }, Outcome::Withdrawn) => {
                Err(InvitationError::Withdrawn)
            }
            (InvitationCommand::Decline { reason }, Outcome::Open) => {
                Ok(vec![InvitationEvent::Declined { reason }])
            }
            (InvitationCommand::Accept { .. }, Outcome::Declined(_)) => {
                Err(InvitationError::Declined)
            }
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

    fn apply(&mut self, event: &InvitationEvent, metadata: &EventMetadata) {
        match event {
            InvitationEvent::Issued { .. } => {}
            InvitationEvent::Accepted => self.settle(Outcome::Accepted, metadata),
            InvitationEvent::Withdrawn => self.settle(Outcome::Withdrawn, metadata),
            InvitationEvent::Declined { reason } => {
                self.settle(Outcome::Declined(*reason), metadata)
            }
            InvitationEvent::Retired => self.retired = true,
            InvitationEvent::Snapshotted {
                to,
                scope,
                by_an_operator,
                issued_at,
                until,
                outcome,
                settled_at,
                retired,
            } => {
                self.to = to.clone();
                self.scope = scope.clone();
                self.by_an_operator = *by_an_operator;
                self.issued_at = *issued_at;
                self.until = *until;
                self.outcome = *outcome;
                self.settled_at = *settled_at;
                self.retired = *retired;
            }
        }
    }

    fn snapshot(&self) -> InvitationEvent {
        InvitationEvent::Snapshotted {
            to: self.to.clone(),
            scope: self.scope.clone(),
            by_an_operator: self.by_an_operator,
            issued_at: self.issued_at,
            until: self.until,
            outcome: self.outcome,
            settled_at: self.settled_at,
            retired: self.retired,
        }
    }
}
