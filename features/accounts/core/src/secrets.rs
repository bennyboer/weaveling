use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;

use crate::code::PendingVerification;
use crate::email::EmailAddress;
use crate::id::AccountId;
use crate::password::PasswordHash;

#[derive(Debug, Error)]
pub enum SecretsError {
    #[error("this address already belongs to an account")]
    Taken,
    #[error(transparent)]
    Backend(Box<dyn Error + Send + Sync>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registered {
    pub account: AccountId,
    pub password: PasswordHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attempt {
    Nothing,
    Exhausted,
    Counted(PendingVerification),
}

#[async_trait]
pub trait Credentials: Send + Sync {
    async fn register(
        &self,
        email: &EmailAddress,
        account: &AccountId,
        password: &PasswordHash,
    ) -> Result<(), SecretsError>;

    async fn registered(&self, email: &EmailAddress) -> Result<Option<Registered>, SecretsError>;

    async fn release(&self, email: &EmailAddress, account: &AccountId) -> Result<(), SecretsError>;
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
