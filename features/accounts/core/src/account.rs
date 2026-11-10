use std::collections::BTreeMap;

use eventsourcing::{Agent, Aggregate, AggregateType, Event, EventMetadata, EventName, Version};
use thiserror::Error;
use time::OffsetDateTime;

use crate::decline::DeclineReason;
use crate::deletion::{DeletionAskedBy, PendingDeletion};
use crate::key::{Key, KeyId, KeyName, KeyRole};
use crate::lock::{Lock, LockKind, LockReason};
use crate::name::AccountName;
use crate::recovery_code::RecoveryCode;
use crate::second_factor::{PreferredWay, SecondFactor, SecondStepWay, Totp};
use crate::session::Stage;
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
const TOTP_ADDED: EventName = EventName::of("TOTP_ADDED");
const RECOVERY_CODES_DRAWN: EventName = EventName::of("RECOVERY_CODES_DRAWN");
const RECOVERY_CODE_USED: EventName = EventName::of("RECOVERY_CODE_USED");
const TOTP_REMOVED: EventName = EventName::of("TOTP_REMOVED");
const KEY_ADDED: EventName = EventName::of("KEY_ADDED");
const KEY_RENAMED: EventName = EventName::of("KEY_RENAMED");
const KEY_REMOVED: EventName = EventName::of("KEY_REMOVED");
const KEY_DEMOTED: EventName = EventName::of("KEY_DEMOTED");
const PREFERRED_WAY_CHOSEN: EventName = EventName::of("PREFERRED_WAY_CHOSEN");
const DELETION_ASKED: EventName = EventName::of("DELETION_ASKED");
const DELETION_CALLED_OFF: EventName = EventName::of("DELETION_CALLED_OFF");
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
    AddTotp,
    DrawRecoveryCodes,
    UseRecoveryCode,
    RemoveTotp,
    AddKey {
        key: KeyId,
        name: KeyName,
        role: KeyRole,
    },
    RenameKey {
        key: KeyId,
        name: KeyName,
    },
    RemoveKey {
        key: KeyId,
    },
    DemoteKey {
        key: KeyId,
    },
    ChoosePreferredWay(PreferredWay),
    AskDeletion {
        until: OffsetDateTime,
        by: DeletionAskedBy,
    },
    CallOffDeletion {
        by: DeletionAskedBy,
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
    TotpAdded,
    RecoveryCodesDrawn,
    RecoveryCodeUsed,
    TotpRemoved,
    KeyAdded {
        key: KeyId,
        name: KeyName,
        role: KeyRole,
    },
    KeyRenamed {
        key: KeyId,
        name: KeyName,
    },
    KeyRemoved {
        key: KeyId,
    },
    KeyDemoted {
        key: KeyId,
    },
    PreferredWayChosen {
        way: PreferredWay,
    },
    DeletionAsked {
        until: OffsetDateTime,
        by: DeletionAskedBy,
    },
    DeletionCalledOff,
    Snapshotted {
        email: EmailAddress,
        name: AccountName,
        verified: bool,
        signed_up_at: OffsetDateTime,
        sessions_since: Option<OffsetDateTime>,
        operator: bool,
        locks: BTreeMap<LockKind, Lock>,
        totp: Option<Totp>,
        keys: BTreeMap<KeyId, Key>,
        recovery_codes_left: usize,
        preferred_way: PreferredWay,
        pending_deletion: Option<PendingDeletion>,
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
    totp: Option<Totp>,
    keys: BTreeMap<KeyId, Key>,
    recovery_codes_left: usize,
    preferred_way: PreferredWay,
    pending_deletion: Option<PendingDeletion>,
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
    #[error("the account holds a key by that id already")]
    KeyAlreadyAdded,
    #[error("the account holds no key by that id")]
    NoSuchKey,
    #[error("another key of the account has that name")]
    KeyNameTaken,
    #[error("an account holds at most {} keys", Account::MOST_KEYS)]
    TooManyKeys,
    #[error(
        "the operator asked for this account to be deleted, and only the operator calls it off"
    )]
    DeletionAskedByTheOperator,
}

impl Account {
    pub const MOST_KEYS: usize = 20;

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
        let second_step_keys = self
            .keys
            .values()
            .filter(|key| key.role == KeyRole::SecondFactor)
            .map(|key| key.since);

        self.totp
            .map(|totp| totp.since)
            .into_iter()
            .chain(second_step_keys)
            .min()
            .map(|since| SecondFactor {
                since,
                recovery_codes_left: self.recovery_codes_left,
                answered_by_totp: self.totp.is_some(),
                answered_by_key: self
                    .keys
                    .values()
                    .any(|key| key.role == KeyRole::SecondFactor),
            })
    }

    /// Whether a key answers the second step besides TOTP.
    pub fn answers_second_step_without_totp(&self) -> bool {
        self.keys
            .values()
            .any(|key| key.role == KeyRole::SecondFactor)
    }

    /// Whether TOTP or another key answers the second step besides this key.
    pub fn answers_second_step_without_key(&self, removed: &KeyId) -> bool {
        self.totp.is_some()
            || self
                .keys
                .iter()
                .any(|(key, held)| key != removed && held.role == KeyRole::SecondFactor)
    }

    pub fn preferred_way(&self) -> PreferredWay {
        self.preferred_way
    }

    /// The way the second step is asked first: the one chosen, else the one that answered last, while it still answers; else a key before the app.
    pub fn asked_way(&self, answered_last: Option<SecondStepWay>) -> Option<SecondStepWay> {
        let answers = |way: &SecondStepWay| match way {
            SecondStepWay::Totp => self.totp.is_some(),
            SecondStepWay::Key => self.answers_second_step_without_totp(),
        };
        let wished = match self.preferred_way {
            PreferredWay::Chosen(way) => Some(way),
            PreferredWay::LastUsed => answered_last,
        };

        wished.filter(answers).or_else(|| {
            [SecondStepWay::Key, SecondStepWay::Totp]
                .into_iter()
                .find(answers)
        })
    }

    pub fn keys(&self) -> &BTreeMap<KeyId, Key> {
        &self.keys
    }

    pub fn totp(&self) -> Option<Totp> {
        self.totp
    }

    /// Whether the account is kept from signing in: locked by the operator, or asked to be deleted.
    pub fn is_locked(&self) -> bool {
        !self.locks.is_empty() || self.pending_deletion.is_some()
    }

    pub fn pending_deletion(&self) -> Option<PendingDeletion> {
        self.pending_deletion
    }

    /// Whether the operator locked the account, which nobody signs in to at all, unlike an account asked to be deleted, signed in to only to be kept.
    pub fn is_locked_by_the_operator(&self) -> bool {
        !self.locks.is_empty()
    }

    /// The stage a session opens at once whatever signing in asks is answered: signed in, or, for an account asked to be deleted, only to keep it or sign out.
    pub fn signs_in_to(&self) -> Stage {
        if self.pending_deletion.is_some() {
            Stage::DeletionPending
        } else {
            Stage::SignedIn
        }
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
            Self::TotpAdded => TOTP_ADDED,
            Self::RecoveryCodesDrawn => RECOVERY_CODES_DRAWN,
            Self::RecoveryCodeUsed => RECOVERY_CODE_USED,
            Self::TotpRemoved => TOTP_REMOVED,
            Self::KeyAdded { .. } => KEY_ADDED,
            Self::KeyRenamed { .. } => KEY_RENAMED,
            Self::KeyRemoved { .. } => KEY_REMOVED,
            Self::KeyDemoted { .. } => KEY_DEMOTED,
            Self::PreferredWayChosen { .. } => PREFERRED_WAY_CHOSEN,
            Self::DeletionAsked { .. } => DELETION_ASKED,
            Self::DeletionCalledOff => DELETION_CALLED_OFF,
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
            | AccountCommand::AddTotp
            | AccountCommand::DrawRecoveryCodes
            | AccountCommand::UseRecoveryCode
            | AccountCommand::RemoveTotp
            | AccountCommand::AddKey { .. }
            | AccountCommand::RenameKey { .. }
            | AccountCommand::RemoveKey { .. }
            | AccountCommand::DemoteKey { .. }
            | AccountCommand::ChoosePreferredWay(_)
            | AccountCommand::AskDeletion { .. }
            | AccountCommand::CallOffDeletion { .. } => Err(AccountError::NotSignedUpYet),
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
                totp: None,
                keys: BTreeMap::new(),
                recovery_codes_left: 0,
                preferred_way: PreferredWay::default(),
                pending_deletion: None,
            }),
            AccountEvent::SignedUpByInvitation { email, name } => Some(Self {
                email: email.clone(),
                name: name.clone(),
                verified: true,
                signed_up_at: metadata.occurred_at,
                sessions_since: None,
                operator: false,
                locks: BTreeMap::new(),
                totp: None,
                keys: BTreeMap::new(),
                recovery_codes_left: 0,
                preferred_way: PreferredWay::default(),
                pending_deletion: None,
            }),
            AccountEvent::Snapshotted {
                email,
                name,
                verified,
                signed_up_at,
                sessions_since,
                operator,
                locks,
                totp,
                keys,
                recovery_codes_left,
                preferred_way,
                pending_deletion,
            } => Some(Self {
                email: email.clone(),
                name: name.clone(),
                verified: *verified,
                signed_up_at: *signed_up_at,
                sessions_since: *sessions_since,
                operator: *operator,
                locks: locks.clone(),
                totp: *totp,
                keys: keys.clone(),
                recovery_codes_left: *recovery_codes_left,
                preferred_way: *preferred_way,
                pending_deletion: *pending_deletion,
            }),
            AccountEvent::EmailVerified
            | AccountEvent::PasswordReset
            | AccountEvent::BecameOperator
            | AccountEvent::StoppedBeingOperator
            | AccountEvent::Locked { .. }
            | AccountEvent::Unlocked { .. }
            | AccountEvent::InvitationDeclined { .. }
            | AccountEvent::InvitationTakenUp { .. }
            | AccountEvent::TotpAdded
            | AccountEvent::RecoveryCodesDrawn
            | AccountEvent::RecoveryCodeUsed
            | AccountEvent::TotpRemoved
            | AccountEvent::KeyAdded { .. }
            | AccountEvent::KeyRenamed { .. }
            | AccountEvent::KeyRemoved { .. }
            | AccountEvent::KeyDemoted { .. }
            | AccountEvent::PreferredWayChosen { .. }
            | AccountEvent::DeletionAsked { .. }
            | AccountEvent::DeletionCalledOff => None,
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
            AccountCommand::BecomeOperator if self.is_locked() => {
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
            AccountCommand::AddTotp => Ok(vec![
                AccountEvent::TotpAdded,
                AccountEvent::RecoveryCodesDrawn,
            ]),
            AccountCommand::DrawRecoveryCodes | AccountCommand::UseRecoveryCode
                if self.second_factor().is_none() =>
            {
                Err(AccountError::NoSecondFactor)
            }
            AccountCommand::DrawRecoveryCodes => Ok(vec![AccountEvent::RecoveryCodesDrawn]),
            AccountCommand::UseRecoveryCode if self.recovery_codes_left == 0 => {
                Err(AccountError::NoRecoveryCodeLeft)
            }
            AccountCommand::UseRecoveryCode => Ok(vec![AccountEvent::RecoveryCodeUsed]),
            AccountCommand::RemoveTotp if self.totp.is_none() => Ok(vec![]),
            AccountCommand::RemoveTotp => Ok(vec![AccountEvent::TotpRemoved]),
            AccountCommand::AddKey { key, .. } if self.keys.contains_key(&key) => {
                Err(AccountError::KeyAlreadyAdded)
            }
            AccountCommand::AddKey { .. } if self.keys.len() >= Self::MOST_KEYS => {
                Err(AccountError::TooManyKeys)
            }
            AccountCommand::AddKey { ref name, .. } if self.names_a_key(name) => {
                Err(AccountError::KeyNameTaken)
            }
            AccountCommand::AddKey { key, name, role } => {
                let first_second_factor =
                    role == KeyRole::SecondFactor && self.second_factor().is_none();
                let added = AccountEvent::KeyAdded { key, name, role };

                Ok(if first_second_factor {
                    vec![added, AccountEvent::RecoveryCodesDrawn]
                } else {
                    vec![added]
                })
            }
            AccountCommand::RenameKey { key, .. } if !self.keys.contains_key(&key) => {
                Err(AccountError::NoSuchKey)
            }
            AccountCommand::RenameKey { key, name } if self.keys[&key].name == name => Ok(vec![]),
            AccountCommand::RenameKey { ref name, .. } if self.names_a_key(name) => {
                Err(AccountError::KeyNameTaken)
            }
            AccountCommand::RenameKey { key, name } => {
                Ok(vec![AccountEvent::KeyRenamed { key, name }])
            }
            AccountCommand::RemoveKey { key } if !self.keys.contains_key(&key) => Ok(vec![]),
            AccountCommand::RemoveKey { key } => Ok(vec![AccountEvent::KeyRemoved { key }]),
            AccountCommand::DemoteKey { key } => match self.keys.get(&key) {
                None => Err(AccountError::NoSuchKey),
                Some(held) if held.role == KeyRole::SecondFactor => Ok(vec![]),
                Some(_) if self.second_factor().is_none() => Ok(vec![
                    AccountEvent::KeyDemoted { key },
                    AccountEvent::RecoveryCodesDrawn,
                ]),
                Some(_) => Ok(vec![AccountEvent::KeyDemoted { key }]),
            },
            AccountCommand::ChoosePreferredWay(way) if way == self.preferred_way => Ok(vec![]),
            AccountCommand::ChoosePreferredWay(way) => {
                Ok(vec![AccountEvent::PreferredWayChosen { way }])
            }
            AccountCommand::AskDeletion { .. } if self.pending_deletion.is_some() => Ok(vec![]),
            AccountCommand::AskDeletion { until, by } if self.operator => Ok(vec![
                AccountEvent::StoppedBeingOperator,
                AccountEvent::DeletionAsked { until, by },
            ]),
            AccountCommand::AskDeletion { until, by } => {
                Ok(vec![AccountEvent::DeletionAsked { until, by }])
            }
            AccountCommand::CallOffDeletion { by } => match self.pending_deletion {
                None => Ok(vec![]),
                Some(pending)
                    if pending.asked_by == DeletionAskedBy::Operator
                        && by == DeletionAskedBy::Holder =>
                {
                    Err(AccountError::DeletionAskedByTheOperator)
                }
                Some(_) => Ok(vec![AccountEvent::DeletionCalledOff]),
            },
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
            AccountEvent::TotpAdded => {
                self.totp = Some(Totp {
                    since: metadata.occurred_at,
                });
                self.sessions_since = Some(metadata.occurred_at);
            }
            AccountEvent::RecoveryCodesDrawn => self.recovery_codes_left = RecoveryCode::DRAWN,
            AccountEvent::RecoveryCodeUsed => {
                self.recovery_codes_left = self.recovery_codes_left.saturating_sub(1);
            }
            AccountEvent::TotpRemoved => {
                self.totp = None;
                self.sessions_since = Some(metadata.occurred_at);
            }
            AccountEvent::KeyAdded { key, name, role } => {
                self.keys.insert(
                    *key,
                    Key {
                        name: name.clone(),
                        role: *role,
                        since: metadata.occurred_at,
                    },
                );
                if *role == KeyRole::SecondFactor {
                    self.sessions_since = Some(metadata.occurred_at);
                }
            }
            AccountEvent::KeyRenamed { key, name } => {
                if let Some(held) = self.keys.get_mut(key) {
                    held.name = name.clone();
                }
            }
            AccountEvent::KeyRemoved { key } => {
                self.keys.remove(key);
                self.sessions_since = Some(metadata.occurred_at);
            }
            AccountEvent::KeyDemoted { key } => {
                if let Some(held) = self.keys.get_mut(key) {
                    held.role = KeyRole::SecondFactor;
                }
                self.sessions_since = Some(metadata.occurred_at);
            }
            AccountEvent::PreferredWayChosen { way } => self.preferred_way = *way,
            AccountEvent::DeletionAsked { until, by } => {
                self.pending_deletion = Some(PendingDeletion {
                    until: *until,
                    asked_by: *by,
                });
                self.sessions_since = Some(metadata.occurred_at);
            }
            AccountEvent::DeletionCalledOff => self.pending_deletion = None,
            AccountEvent::Snapshotted {
                email,
                name,
                verified,
                signed_up_at,
                sessions_since,
                operator,
                locks,
                totp,
                keys,
                recovery_codes_left,
                preferred_way,
                pending_deletion,
            } => {
                self.email = email.clone();
                self.name = name.clone();
                self.verified = *verified;
                self.signed_up_at = *signed_up_at;
                self.sessions_since = *sessions_since;
                self.operator = *operator;
                self.locks = locks.clone();
                self.totp = *totp;
                self.keys = keys.clone();
                self.recovery_codes_left = *recovery_codes_left;
                self.preferred_way = *preferred_way;
                self.pending_deletion = *pending_deletion;
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
            totp: self.totp,
            keys: self.keys.clone(),
            recovery_codes_left: self.recovery_codes_left,
            preferred_way: self.preferred_way,
            pending_deletion: self.pending_deletion,
        }
    }
}

impl Account {
    fn names_a_key(&self, name: &KeyName) -> bool {
        self.keys.values().any(|held| held.name == *name)
    }
}
