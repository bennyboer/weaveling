use std::collections::BTreeMap;

use eventsourcing::{Agent, Aggregate, AggregateType, Event, EventMetadata, EventName, Version};
use thiserror::Error;
use time::OffsetDateTime;

use crate::decline::DeclineReason;
use crate::lock::{Lock, LockKind, LockReason};
use crate::name::AccountName;
use email::EmailAddress;

pub const KIND: AggregateType = AggregateType::of("account");

const SIGNED_UP: EventName = EventName::of("SIGNED_UP");
const SIGNED_UP_BY_INVITATION: EventName = EventName::of("SIGNED_UP_BY_INVITATION");
const INVITATION_DECLINED: EventName = EventName::of("INVITATION_DECLINED");
const INVITATION_TAKEN_UP: EventName = EventName::of("INVITATION_TAKEN_UP");
const EMAIL_VERIFIED: EventName = EventName::of("EMAIL_VERIFIED");
const PASSWORD_RESET: EventName = EventName::of("PASSWORD_RESET");
const BECAME_OPERATOR: EventName = EventName::of("BECAME_OPERATOR");
const STOPPED_BEING_OPERATOR: EventName = EventName::of("STOPPED_BEING_OPERATOR");
const LOCKED: EventName = EventName::of("LOCKED");
const UNLOCKED: EventName = EventName::of("UNLOCKED");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountCommand {
    SignUp(EmailAddress, AccountName),
    SignUpByInvitation {
        email: EmailAddress,
        name: AccountName,
        invitation: String,
    },
    Verify,
    ResetPassword,
    BecomeOperator,
    StopBeingOperator,
    Lock(LockKind, LockReason),
    Unlock(LockKind),
    DeclineInvitation {
        invitation: String,
        reason: DeclineReason,
    },
    TakeUpInvitation {
        invitation: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountEvent {
    SignedUp {
        email: EmailAddress,
        name: AccountName,
    },
    SignedUpByInvitation {
        email: EmailAddress,
        name: AccountName,
    },
    EmailVerified,
    PasswordReset,
    BecameOperator,
    StoppedBeingOperator,
    Locked {
        kind: LockKind,
        reason: LockReason,
    },
    Unlocked {
        kind: LockKind,
    },
    InvitationDeclined {
        invitation: String,
        reason: DeclineReason,
    },
    InvitationTakenUp {
        invitation: String,
    },
    Snapshotted {
        email: EmailAddress,
        name: AccountName,
        verified: bool,
        signed_up_at: OffsetDateTime,
        sessions_since: Option<OffsetDateTime>,
        operator: bool,
        locks: BTreeMap<LockKind, Lock>,
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
    locks: BTreeMap<LockKind, Lock>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AccountError {
    #[error("an account must be signed up before anything else can happen to it")]
    NotSignedUpYet,
    #[error("an account cannot be signed up twice")]
    AlreadySignedUp,
    #[error("an account must have its address verified before it can become an operator")]
    NotVerifiedYet,
    #[error("an operator cannot be locked; the role is to be taken away first")]
    OperatorCannotBeLocked,
    #[error("a locked account cannot become an operator")]
    LockedCannotBecomeOperator,
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

    pub fn locks(&self) -> &BTreeMap<LockKind, Lock> {
        &self.locks
    }

    pub fn is_locked(&self) -> bool {
        !self.locks.is_empty()
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
            Self::SignedUpByInvitation { .. } => SIGNED_UP_BY_INVITATION,
            Self::EmailVerified => EMAIL_VERIFIED,
            Self::PasswordReset => PASSWORD_RESET,
            Self::BecameOperator => BECAME_OPERATOR,
            Self::StoppedBeingOperator => STOPPED_BEING_OPERATOR,
            Self::Locked { .. } => LOCKED,
            Self::Unlocked { .. } => UNLOCKED,
            Self::InvitationDeclined { .. } => INVITATION_DECLINED,
            Self::InvitationTakenUp { .. } => INVITATION_TAKEN_UP,
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
            AccountCommand::SignUpByInvitation {
                email,
                name,
                invitation,
            } => Ok(vec![
                AccountEvent::SignedUpByInvitation { email, name },
                AccountEvent::InvitationTakenUp { invitation },
            ]),
            AccountCommand::Verify
            | AccountCommand::ResetPassword
            | AccountCommand::BecomeOperator
            | AccountCommand::StopBeingOperator
            | AccountCommand::Lock(..)
            | AccountCommand::Unlock(_)
            | AccountCommand::DeclineInvitation { .. }
            | AccountCommand::TakeUpInvitation { .. } => Err(AccountError::NotSignedUpYet),
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
                locks: BTreeMap::new(),
            }),
            AccountEvent::SignedUpByInvitation { email, name } => Some(Self {
                email: email.clone(),
                name: name.clone(),
                verified: true,
                signed_up_at: metadata.occurred_at,
                sessions_since: None,
                operator: false,
                locks: BTreeMap::new(),
            }),
            AccountEvent::Snapshotted {
                email,
                name,
                verified,
                signed_up_at,
                sessions_since,
                operator,
                locks,
            } => Some(Self {
                email: email.clone(),
                name: name.clone(),
                verified: *verified,
                signed_up_at: *signed_up_at,
                sessions_since: *sessions_since,
                operator: *operator,
                locks: locks.clone(),
            }),
            AccountEvent::EmailVerified
            | AccountEvent::PasswordReset
            | AccountEvent::BecameOperator
            | AccountEvent::StoppedBeingOperator
            | AccountEvent::Locked { .. }
            | AccountEvent::Unlocked { .. }
            | AccountEvent::InvitationDeclined { .. }
            | AccountEvent::InvitationTakenUp { .. } => None,
        }
    }

    fn decide(
        &self,
        command: AccountCommand,
        _agent: &Agent,
    ) -> Result<Vec<AccountEvent>, AccountError> {
        match command {
            AccountCommand::SignUp(..) | AccountCommand::SignUpByInvitation { .. } => {
                Err(AccountError::AlreadySignedUp)
            }
            AccountCommand::Verify if self.verified => Ok(vec![]),
            AccountCommand::Verify => Ok(vec![AccountEvent::EmailVerified]),
            AccountCommand::ResetPassword if self.verified => Ok(vec![AccountEvent::PasswordReset]),
            AccountCommand::ResetPassword => Ok(vec![
                AccountEvent::EmailVerified,
                AccountEvent::PasswordReset,
            ]),
            AccountCommand::BecomeOperator if !self.verified => Err(AccountError::NotVerifiedYet),
            AccountCommand::BecomeOperator if self.operator => Ok(vec![]),
            AccountCommand::BecomeOperator if !self.locks.is_empty() => {
                Err(AccountError::LockedCannotBecomeOperator)
            }
            AccountCommand::BecomeOperator => Ok(vec![AccountEvent::BecameOperator]),
            AccountCommand::StopBeingOperator if !self.operator => Ok(vec![]),
            AccountCommand::StopBeingOperator => Ok(vec![AccountEvent::StoppedBeingOperator]),
            AccountCommand::Lock(..) if self.operator => Err(AccountError::OperatorCannotBeLocked),
            AccountCommand::Lock(kind, _) if self.locks.contains_key(&kind) => Ok(vec![]),
            AccountCommand::Lock(kind, reason) => Ok(vec![AccountEvent::Locked { kind, reason }]),
            AccountCommand::Unlock(kind) if !self.locks.contains_key(&kind) => Ok(vec![]),
            AccountCommand::Unlock(kind) => Ok(vec![AccountEvent::Unlocked { kind }]),
            AccountCommand::DeclineInvitation { invitation, reason } => {
                Ok(vec![AccountEvent::InvitationDeclined {
                    invitation,
                    reason,
                }])
            }
            AccountCommand::TakeUpInvitation { invitation } => {
                Ok(vec![AccountEvent::InvitationTakenUp { invitation }])
            }
        }
    }

    fn apply(&mut self, event: &AccountEvent, metadata: &EventMetadata) {
        match event {
            AccountEvent::SignedUp { .. } | AccountEvent::SignedUpByInvitation { .. } => {}
            AccountEvent::EmailVerified => self.verified = true,
            AccountEvent::PasswordReset => self.sessions_since = Some(metadata.occurred_at),
            AccountEvent::BecameOperator => self.operator = true,
            AccountEvent::StoppedBeingOperator => self.operator = false,
            AccountEvent::Locked { kind, reason } => {
                self.locks.insert(
                    *kind,
                    Lock {
                        reason: reason.clone(),
                        since: metadata.occurred_at,
                    },
                );
                self.sessions_since = Some(metadata.occurred_at);
            }
            AccountEvent::Unlocked { kind } => {
                self.locks.remove(kind);
            }
            AccountEvent::InvitationDeclined { .. } | AccountEvent::InvitationTakenUp { .. } => {}
            AccountEvent::Snapshotted {
                email,
                name,
                verified,
                signed_up_at,
                sessions_since,
                operator,
                locks,
            } => {
                self.email = email.clone();
                self.name = name.clone();
                self.verified = *verified;
                self.signed_up_at = *signed_up_at;
                self.sessions_since = *sessions_since;
                self.operator = *operator;
                self.locks = locks.clone();
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
            locks: self.locks.clone(),
        }
    }
}
