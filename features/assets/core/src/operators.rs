use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use transacting::Transaction;

use crate::catalog::CatalogError;

/// Who runs the instance, kept by the assets feature itself as the accounts feature tells it, so what only an operator may do is asked of it alone. Each word comes with the account's version it was told at, the newest heard winning, so one heard late or twice changes nothing.
#[async_trait]
pub trait OperatorCatalog: Send + Sync {
    /// Remembers whether the account runs the instance, as of a version of it; an account deleted runs it no more.
    async fn remember(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        version: Version,
        operator: bool,
    ) -> Result<(), CatalogError>;

    /// Whether the account runs the instance.
    async fn operates(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<bool, CatalogError>;
}
