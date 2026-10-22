use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;

use crate::code::PendingVerification;
use crate::email::EmailAddress;
use crate::id::AccountId;
use crate::password::PasswordHash;
use crate::session::{Session, TokenDigest};

#[derive(Debug, Error)]
pub enum SecretsError {
    #[error("this address already belongs to an account")]
    Taken,
    #[error(transparent)]
    Backend(Box<dyn Error + Send + Sync>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attempt {
    Nothing,
    Exhausted,
    Counted(PendingVerification),
}

#[async_trait]
pub trait EmailAddresses: Send + Sync {
    async fn claim(&self, email: &EmailAddress, account: &AccountId) -> Result<(), SecretsError>;

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
        pending: &PendingVerification,
    ) -> Result<(), SecretsError>;

    async fn attempt(&self, account: &AccountId, within: u32) -> Result<Attempt, SecretsError>;

    async fn close(&self, account: &AccountId) -> Result<(), SecretsError>;
}
