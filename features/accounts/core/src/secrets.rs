use std::error::Error;
use std::sync::Arc;
use transacting::Transaction;

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;

use crate::code::{CodePurpose, PendingVerification, VerificationCode};
use crate::id::AccountId;
use crate::password::PasswordHash;
use crate::recovery_code::RecoveryCodeHash;
use crate::session::{Session, TokenDigest};
use crate::ticket::InvitationTicket;
use crate::totp::TotpStep;
use crate::totp_key::SealedTotpSecret;
use email::EmailAddress;
use throttling::Throttles;

#[derive(Debug, Error)]
pub enum SecretsError {
    #[error("this address already belongs to an account")]
    Taken,
    #[error(transparent)]
    Backend(Box<dyn Error + Send + Sync>),
}

#[derive(Clone)]
pub struct SecretStores {
    pub email_addresses: Arc<dyn EmailAddresses>,
    pub passwords: Arc<dyn Passwords>,
    pub verifications: Arc<dyn Verifications>,
    pub sessions: Arc<dyn Sessions>,
    pub throttles: Arc<dyn Throttles>,
    pub operators: Arc<dyn Operators>,
    pub invitation_tickets: Arc<dyn InvitationTickets>,
    pub second_factors: Arc<dyn SecondFactors>,
}

impl SecretStores {
    pub fn kept_by<S>(secrets: Arc<S>, throttles: Arc<dyn Throttles>) -> Self
    where
        S: EmailAddresses
            + Passwords
            + Verifications
            + Sessions
            + Operators
            + InvitationTickets
            + SecondFactors
            + 'static,
    {
        Self {
            email_addresses: secrets.clone(),
            passwords: secrets.clone(),
            verifications: secrets.clone(),
            sessions: secrets.clone(),
            throttles,
            operators: secrets.clone(),
            invitation_tickets: secrets.clone(),
            second_factors: secrets,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attempt {
    Nothing,
    Exhausted,
    Counted(PendingVerification),
}

#[async_trait]
pub trait EmailAddresses: Send + Sync {
    async fn claim(
        &self,
        transaction: &mut Transaction,
        email: &EmailAddress,
        account: &AccountId,
        at: OffsetDateTime,
    ) -> Result<(), SecretsError>;

    async fn take_over(
        &self,
        transaction: &mut Transaction,
        email: &EmailAddress,
        from: &AccountId,
        to: &AccountId,
        claimed_before: OffsetDateTime,
        at: OffsetDateTime,
    ) -> Result<bool, SecretsError>;

    async fn holder(&self, email: &EmailAddress) -> Result<Option<AccountId>, SecretsError>;

    async fn release(
        &self,
        transaction: &mut Transaction,
        email: &EmailAddress,
        account: &AccountId,
    ) -> Result<(), SecretsError>;
}

#[async_trait]
pub trait Passwords: Send + Sync {
    async fn keep(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        password: &PasswordHash,
    ) -> Result<(), SecretsError>;

    async fn of(&self, account: &AccountId) -> Result<Option<PasswordHash>, SecretsError>;
}

#[async_trait]
pub trait Sessions: Send + Sync {
    async fn open(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
        session: &Session,
    ) -> Result<(), SecretsError>;

    async fn find(&self, digest: &TokenDigest) -> Result<Option<Session>, SecretsError>;

    async fn end(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<(), SecretsError>;
}

#[async_trait]
pub trait Verifications: Send + Sync {
    async fn issue(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        purpose: CodePurpose,
        pending: &PendingVerification,
    ) -> Result<(), SecretsError>;

    async fn attempt(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        purpose: CodePurpose,
        within: u32,
    ) -> Result<Attempt, SecretsError>;

    async fn close(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        purpose: CodePurpose,
        code: &VerificationCode,
    ) -> Result<bool, SecretsError>;
}

#[async_trait]
pub trait Operators: Send + Sync {
    async fn keep_code(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<bool, SecretsError>;

    async fn holds_code(&self, digest: &TokenDigest) -> Result<bool, SecretsError>;

    async fn take_code(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<bool, SecretsError>;

    async fn appoint(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError>;

    async fn dismiss(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<Dismissal, SecretsError>;

    async fn is_appointed(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<bool, SecretsError>;

    async fn any_appointed(&self) -> Result<bool, SecretsError>;
}

#[async_trait]
pub trait InvitationTickets: Send + Sync {
    async fn keep_ticket(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
        ticket: &InvitationTicket,
    ) -> Result<bool, SecretsError>;

    async fn find_ticket(
        &self,
        digest: &TokenDigest,
    ) -> Result<Option<InvitationTicket>, SecretsError>;

    async fn take_ticket(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<Option<InvitationTicket>, SecretsError>;

    async fn digest_of_ticket(&self, invitation: &str)
    -> Result<Option<TokenDigest>, SecretsError>;

    async fn forget_ticket(
        &self,
        transaction: &mut Transaction,
        invitation: &str,
    ) -> Result<(), SecretsError>;

    async fn drop_ticket(
        &self,
        transaction: &mut Transaction,
        invitation: &str,
    ) -> Result<(), SecretsError>;

    async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: u64,
        in_the_trash: bool,
    ) -> Result<(), SecretsError>;

    async fn forget_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
    ) -> Result<(), SecretsError>;

    async fn is_in_the_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
    ) -> Result<bool, SecretsError>;
}

/// TOTP as it is kept: its sealed secret, and the step of the last code it accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptTotp {
    pub secret: SealedTotpSecret,
    pub last_accepted: TotpStep,
}

/// An account's second-factor secrets: a TOTP secret being set up until a code of it is confirmed, then the one in use for signing in, which setting up anew leaves in use until the new one is finished; and its recovery codes, kept hashed.
#[async_trait]
pub trait SecondFactors: Send + Sync {
    async fn begin_setting_up(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        secret: &SealedTotpSecret,
    ) -> Result<(), SecretsError>;

    async fn totp_being_set_up(
        &self,
        account: &AccountId,
    ) -> Result<Option<SealedTotpSecret>, SecretsError>;

    async fn finish_setting_up(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        being_set_up: &SealedTotpSecret,
        accepted: TotpStep,
    ) -> Result<bool, SecretsError>;

    async fn totp_of(&self, account: &AccountId) -> Result<Option<KeptTotp>, SecretsError>;

    async fn accept_step(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        step: TotpStep,
    ) -> Result<bool, SecretsError>;

    async fn keep_recovery_codes(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        codes: &[RecoveryCodeHash],
    ) -> Result<(), SecretsError>;

    async fn recovery_codes_of(
        &self,
        account: &AccountId,
    ) -> Result<Vec<RecoveryCodeHash>, SecretsError>;

    async fn use_recovery_code(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        code: &RecoveryCodeHash,
    ) -> Result<bool, SecretsError>;

    async fn forget_totp(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dismissal {
    Dismissed,
    LastOne,
    NotAppointed,
}
