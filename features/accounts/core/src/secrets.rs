use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;

use crate::code::{CodePurpose, PendingVerification};
use crate::id::AccountId;
use crate::password::PasswordHash;
use crate::session::{Session, TokenDigest};
use crate::ticket::InvitationTicket;
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
            + 'static,
    {
        Self {
            email_addresses: secrets.clone(),
            passwords: secrets.clone(),
            verifications: secrets.clone(),
            sessions: secrets.clone(),
            throttles,
            operators: secrets.clone(),
            invitation_tickets: secrets,
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
        email: &EmailAddress,
        account: &AccountId,
        at: OffsetDateTime,
    ) -> Result<(), SecretsError>;

    async fn take_over(
        &self,
        email: &EmailAddress,
        from: &AccountId,
        to: &AccountId,
        claimed_before: OffsetDateTime,
        at: OffsetDateTime,
    ) -> Result<bool, SecretsError>;

    async fn holder(&self, email: &EmailAddress) -> Result<Option<AccountId>, SecretsError>;

    async fn release(&self, email: &EmailAddress, account: &AccountId) -> Result<(), SecretsError>;
}

#[async_trait]
pub trait Passwords: Send + Sync {
    async fn keep(&self, account: &AccountId, password: &PasswordHash) -> Result<(), SecretsError>;

    async fn of(&self, account: &AccountId) -> Result<Option<PasswordHash>, SecretsError>;
}

#[async_trait]
pub trait Sessions: Send + Sync {
    async fn open(&self, digest: &TokenDigest, session: &Session) -> Result<(), SecretsError>;

    async fn find(&self, digest: &TokenDigest) -> Result<Option<Session>, SecretsError>;

    async fn end(&self, digest: &TokenDigest) -> Result<(), SecretsError>;
}

#[async_trait]
pub trait Verifications: Send + Sync {
    async fn issue(
        &self,
        account: &AccountId,
        purpose: CodePurpose,
        pending: &PendingVerification,
    ) -> Result<(), SecretsError>;

    async fn attempt(
        &self,
        account: &AccountId,
        purpose: CodePurpose,
        within: u32,
    ) -> Result<Attempt, SecretsError>;

    async fn close(&self, account: &AccountId, purpose: CodePurpose) -> Result<(), SecretsError>;
}

#[async_trait]
pub trait Operators: Send + Sync {
    async fn keep_code(&self, digest: &TokenDigest) -> Result<(), SecretsError>;

    async fn holds_code(&self, digest: &TokenDigest) -> Result<bool, SecretsError>;

    async fn take_code(&self, digest: &TokenDigest) -> Result<bool, SecretsError>;

    async fn appoint(&self, account: &AccountId) -> Result<(), SecretsError>;

    async fn dismiss(&self, account: &AccountId) -> Result<Dismissal, SecretsError>;

    async fn any_appointed(&self) -> Result<bool, SecretsError>;
}

#[async_trait]
pub trait InvitationTickets: Send + Sync {
    async fn keep_ticket(
        &self,
        digest: &TokenDigest,
        ticket: &InvitationTicket,
    ) -> Result<bool, SecretsError>;

    async fn find_ticket(
        &self,
        digest: &TokenDigest,
    ) -> Result<Option<InvitationTicket>, SecretsError>;

    async fn take_ticket(
        &self,
        digest: &TokenDigest,
    ) -> Result<Option<InvitationTicket>, SecretsError>;

    async fn digest_of_ticket(&self, invitation: &str)
    -> Result<Option<TokenDigest>, SecretsError>;

    async fn forget_ticket(&self, invitation: &str) -> Result<(), SecretsError>;

    async fn drop_ticket(&self, invitation: &str) -> Result<(), SecretsError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dismissal {
    Dismissed,
    LastOne,
    NotAppointed,
}
