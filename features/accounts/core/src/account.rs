use eventsourcing::{Agent, Aggregate, AggregateType, Event, EventMetadata, EventName, Version};
use thiserror::Error;
use time::OffsetDateTime;

use crate::email::EmailAddress;
use crate::name::AccountName;

pub const KIND: AggregateType = AggregateType::of("account");

const SIGNED_UP: EventName = EventName::of("SIGNED_UP");
const EMAIL_VERIFIED: EventName = EventName::of("EMAIL_VERIFIED");
const PASSWORD_RESET: EventName = EventName::of("PASSWORD_RESET");
const BECAME_OPERATOR: EventName = EventName::of("BECAME_OPERATOR");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountCommand {
    SignUp(EmailAddress, AccountName),
    Verify,
    ResetPassword,
    BecomeOperator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountEvent {
    SignedUp {
        email: EmailAddress,
        name: AccountName,
    },
    EmailVerified,
    PasswordReset,
    BecameOperator,
    Snapshotted {
        email: EmailAddress,
        name: AccountName,
        verified: bool,
        signed_up_at: OffsetDateTime,
        sessions_since: Option<OffsetDateTime>,
        operator: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    email: EmailAddress,
    name: AccountName,
    verified: bool,
    signed_up_at: OffsetDateTime,
    sessions_since: Option<OffsetDateTime>,
    operator: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AccountError {
    #[error("an account must be signed up before anything else can happen to it")]
    NotSignedUpYet,
    #[error("an account cannot be signed up twice")]
    AlreadySignedUp,
    #[error("an account must have its address verified before it can become an operator")]
    NotVerifiedYet,
}

impl Account {
    pub fn email(&self) -> &EmailAddress {
        &self.email
    }

    pub fn name(&self) -> &AccountName {
        &self.name
    }

    pub fn is_verified(&self) -> bool {
        self.verified
    }

    pub fn is_operator(&self) -> bool {
        self.operator
    }

    pub fn signed_up_at(&self) -> OffsetDateTime {
        self.signed_up_at
    }

    pub fn counts_session_begun(&self, begun: OffsetDateTime) -> bool {
        self.sessions_since.is_none_or(|since| begun >= since)
    }
}

impl Event for AccountEvent {
    fn name(&self) -> EventName {
        match self {
            Self::SignedUp { .. } => SIGNED_UP,
            Self::EmailVerified => EMAIL_VERIFIED,
            Self::PasswordReset => PASSWORD_RESET,
            Self::BecameOperator => BECAME_OPERATOR,
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

impl Aggregate for Account {
    type Command = AccountCommand;
    type Event = AccountEvent;
    type Error = AccountError;

    const KIND: AggregateType = KIND;

    fn begin(command: AccountCommand, _agent: &Agent) -> Result<Vec<AccountEvent>, AccountError> {
        match command {
            AccountCommand::SignUp(email, name) => Ok(vec![AccountEvent::SignedUp { email, name }]),
            AccountCommand::Verify
            | AccountCommand::ResetPassword
            | AccountCommand::BecomeOperator => Err(AccountError::NotSignedUpYet),
        }
    }

    fn from_first(event: &AccountEvent, metadata: &EventMetadata) -> Option<Self> {
        match event {
            AccountEvent::SignedUp { email, name } => Some(Self {
                email: email.clone(),
                name: name.clone(),
                verified: false,
                signed_up_at: metadata.occurred_at,
                sessions_since: None,
                operator: false,
            }),
            AccountEvent::Snapshotted {
                email,
                name,
                verified,
                signed_up_at,
                sessions_since,
                operator,
            } => Some(Self {
                email: email.clone(),
                name: name.clone(),
                verified: *verified,
                signed_up_at: *signed_up_at,
                sessions_since: *sessions_since,
                operator: *operator,
            }),
            AccountEvent::EmailVerified
            | AccountEvent::PasswordReset
            | AccountEvent::BecameOperator => None,
        }
    }

    fn decide(
        &self,
        command: AccountCommand,
        _agent: &Agent,
    ) -> Result<Vec<AccountEvent>, AccountError> {
        match command {
            AccountCommand::SignUp(..) => Err(AccountError::AlreadySignedUp),
            AccountCommand::Verify if self.verified => Ok(vec![]),
            AccountCommand::Verify => Ok(vec![AccountEvent::EmailVerified]),
            AccountCommand::ResetPassword if self.verified => Ok(vec![AccountEvent::PasswordReset]),
            AccountCommand::ResetPassword => Ok(vec![
                AccountEvent::EmailVerified,
                AccountEvent::PasswordReset,
            ]),
            AccountCommand::BecomeOperator if !self.verified => Err(AccountError::NotVerifiedYet),
            AccountCommand::BecomeOperator if self.operator => Ok(vec![]),
            AccountCommand::BecomeOperator => Ok(vec![AccountEvent::BecameOperator]),
        }
    }

    fn apply(&mut self, event: &AccountEvent, metadata: &EventMetadata) {
        match event {
            AccountEvent::SignedUp { .. } => {}
            AccountEvent::EmailVerified => self.verified = true,
            AccountEvent::PasswordReset => self.sessions_since = Some(metadata.occurred_at),
            AccountEvent::BecameOperator => self.operator = true,
            AccountEvent::Snapshotted {
                email,
                name,
                verified,
                signed_up_at,
                sessions_since,
                operator,
            } => {
                self.email = email.clone();
                self.name = name.clone();
                self.verified = *verified;
                self.signed_up_at = *signed_up_at;
                self.sessions_since = *sessions_since;
                self.operator = *operator;
            }
        }
    }

    fn snapshot(&self) -> AccountEvent {
        AccountEvent::Snapshotted {
            email: self.email.clone(),
            name: self.name.clone(),
            verified: self.verified,
            signed_up_at: self.signed_up_at,
            sessions_since: self.sessions_since,
            operator: self.operator,
        }
    }
}
