use async_trait::async_trait;
use eventsourcing::AccountId;
use transacting::Transaction;

use crate::catalog::CatalogError;

/// The limits an operator gave to single accounts, in bytes, stored by the assets feature.
/// Such a limit replaces the default limit for accounts.
#[async_trait]
pub trait AccountLimits: Send + Sync {
    /// Returns the limit given to the account.
    /// Returns none if the account has the default limit for accounts.
    async fn limit_of(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<Option<u64>, CatalogError>;

    /// Gives the account its own limit, replacing any limit it had before.
    async fn give(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        most: u64,
    ) -> Result<(), CatalogError>;

    /// Removes the account's own limit, so the default limit for accounts applies again.
    /// Removing a limit that was never given does not fail.
    async fn take_back(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), CatalogError>;
}
