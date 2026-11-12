use async_trait::async_trait;
use eventsourcing::AccountId;
use transacting::Transaction;

use crate::catalog::CatalogError;

/// The limits the operator gave single accounts in place of the one each account is held to, in bytes, kept by the assets feature.
#[async_trait]
pub trait AccountLimits: Send + Sync {
    /// The limit given the account, or none where it is held to the one of every account.
    async fn limit_of(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<Option<u64>, CatalogError>;

    /// Gives the account a limit of its own, in place of any it had.
    async fn give(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        most: u64,
    ) -> Result<(), CatalogError>;

    /// Takes the account's own limit back, holding it to the one of every account again; taking back one never given is no failure.
    async fn take_back(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), CatalogError>;
}
