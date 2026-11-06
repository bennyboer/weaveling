use std::collections::BTreeMap;

use eventsourcing::{Agent, Aggregate, AggregateType, Event, EventMetadata, EventName, Version};
use thiserror::Error;
use time::OffsetDateTime;

use crate::decline::DeclineReason;
use crate::lock::{Lock, LockKind, LockReason};
use crate::name::AccountName;
use crate::recovery_code::RecoveryCode;
use crate::second_factor::SecondFactor;
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
const SECOND_FACTOR_ADDED: EventName = EventName::of("SECOND_FACTOR_ADDED");
const RECOVERY_CODES_DRAWN: EventName = EventName::of("RECOVERY_CODES_DRAWN");
const RECOVERY_CODE_USED: EventName = EventName::of("RECOVERY_CODE_USED");
const SECOND_FACTOR_REMOVED: EventName = EventName::of("SECOND_FACTOR_REMOVED");
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
    AddSecondFactor,
    DrawRecoveryCodes,
    UseRecoveryCode,
    RemoveSecondFactor,
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
    SecondFactorAdded,
    RecoveryCodesDrawn,
    RecoveryCodeUsed,
    SecondFactorRemoved,
    Snapshotted {
        email: EmailAddress,
        name: AccountName,
        verified: bool,
        signed_up_at: OffsetDateTime,
        sessions_since: Option<OffsetDateTime>,
        operator: bool,
        locks: BTreeMap<LockKind, Lock>,
        second_factor: Option<SecondFactor>,
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
    second_factor: Option<SecondFactor>,
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
    #[error("the account has no second factor")]
    NoSecondFactor,
    #[error("the account has no recovery code left")]
    NoRecoveryCodeLeft,
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

    pub fn second_factor(&self) -> Option<SecondFactor> {
        self.second_factor
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
            Self::SecondFactorAdded => SECOND_FACTOR_ADDED,
            Self::RecoveryCodesDrawn => RECOVERY_CODES_DRAWN,
            Self::RecoveryCodeUsed => RECOVERY_CODE_USED,
            Self::SecondFactorRemoved => SECOND_FACTOR_REMOVED,
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
            | AccountCommand::TakeUpInvitation { .. }
            | AccountCommand::AddSecondFactor
            | AccountCommand::DrawRecoveryCodes
            | AccountCommand::UseRecoveryCode
            | AccountCommand::RemoveSecondFactor => Err(AccountError::NotSignedUpYet),
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
                second_factor: None,
            }),
            AccountEvent::SignedUpByInvitation { email, name } => Some(Self {
                email: email.clone(),
                name: name.clone(),
                verified: true,
                signed_up_at: metadata.occurred_at,
                sessions_since: None,
                operator: false,
                locks: BTreeMap::new(),
                second_factor: None,
            }),
            AccountEvent::Snapshotted {
                email,
                name,
                verified,
                signed_up_at,
                sessions_since,
                operator,
                locks,
                second_factor,
            } => Some(Self {
                email: email.clone(),
                name: name.clone(),
                verified: *verified,
                signed_up_at: *signed_up_at,
                sessions_since: *sessions_since,
                operator: *operator,
                locks: locks.clone(),
                second_factor: *second_factor,
            }),
            AccountEvent::EmailVerified
            | AccountEvent::PasswordReset
            | AccountEvent::BecameOperator
            | AccountEvent::StoppedBeingOperator
            | AccountEvent::Locked { .. }
            | AccountEvent::Unlocked { .. }
            | AccountEvent::InvitationDeclined { .. }
            | AccountEvent::InvitationTakenUp { .. }
            | AccountEvent::SecondFactorAdded
            | AccountEvent::RecoveryCodesDrawn
            | AccountEvent::RecoveryCodeUsed
            | AccountEvent::SecondFactorRemoved => None,
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
            AccountCommand::AddSecondFactor => Ok(vec![
                AccountEvent::SecondFactorAdded,
                AccountEvent::RecoveryCodesDrawn,
            ]),
            AccountCommand::DrawRecoveryCodes | AccountCommand::UseRecoveryCode
                if self.second_factor.is_none() =>
            {
                Err(AccountError::NoSecondFactor)
            }
            AccountCommand::DrawRecoveryCodes => Ok(vec![AccountEvent::RecoveryCodesDrawn]),
            AccountCommand::UseRecoveryCode
                if self
                    .second_factor
                    .is_some_and(|factor| factor.recovery_codes_left == 0) =>
            {
                Err(AccountError::NoRecoveryCodeLeft)
            }
            AccountCommand::UseRecoveryCode => Ok(vec![AccountEvent::RecoveryCodeUsed]),
            AccountCommand::RemoveSecondFactor if self.second_factor.is_none() => Ok(vec![]),
            AccountCommand::RemoveSecondFactor => Ok(vec![AccountEvent::SecondFactorRemoved]),
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
            AccountEvent::SecondFactorAdded => {
                self.second_factor = Some(SecondFactor {
                    since: metadata.occurred_at,
                    recovery_codes_left: 0,
                });
                self.sessions_since = Some(metadata.occurred_at);
            }
            AccountEvent::RecoveryCodesDrawn => {
                if let Some(factor) = &mut self.second_factor {
                    factor.recovery_codes_left = RecoveryCode::DRAWN;
                }
            }
            AccountEvent::RecoveryCodeUsed => {
                if let Some(factor) = &mut self.second_factor {
                    factor.recovery_codes_left = factor.recovery_codes_left.saturating_sub(1);
                }
            }
            AccountEvent::SecondFactorRemoved => {
                self.second_factor = None;
                self.sessions_since = Some(metadata.occurred_at);
            }
            AccountEvent::Snapshotted {
                email,
                name,
                verified,
                signed_up_at,
                sessions_since,
                operator,
                locks,
                second_factor,
            } => {
                self.email = email.clone();
                self.name = name.clone();
                self.verified = *verified;
                self.signed_up_at = *signed_up_at;
                self.sessions_since = *sessions_since;
                self.operator = *operator;
                self.locks = locks.clone();
                self.second_factor = *second_factor;
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
            second_factor: self.second_factor,
        }
    }
}
