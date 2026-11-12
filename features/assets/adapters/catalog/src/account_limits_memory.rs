use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use assets_core::{AccountLimits, CatalogError};
use async_trait::async_trait;
use eventsourcing::AccountId;
use transacting::{Transaction, memory};

type Kept = HashMap<String, u64>;

#[derive(Debug, Default)]
pub struct InMemoryAccountLimits {
    kept: Arc<Mutex<Kept>>,
}

impl InMemoryAccountLimits {
    pub fn new() -> Self {
        Self::default()
    }

    fn kept(&self) -> MutexGuard<'_, Kept> {
        self.kept
            .lock()
            .expect("the account limits are not poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let before = self.kept().clone();
        let into = self.kept.clone();

        memory::undo_on_rollback(transaction, move || {
            *into.lock().expect("the account limits are not poisoned") = before;
        })
        .map_err(|failure| CatalogError::Backend(Box::new(failure)))
    }
}

#[async_trait]
impl AccountLimits for InMemoryAccountLimits {
    async fn limit_of(
        &self,
        _transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<Option<u64>, CatalogError> {
        Ok(self.kept().get(&account.to_string()).copied())
    }

    async fn give(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        most: u64,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.kept().insert(account.to_string(), most);

        Ok(())
    }

    async fn take_back(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.kept().remove(&account.to_string());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;
    use crate::account_limits_suite::Workbench;

    struct InMemory(InMemoryAccountLimits, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryAccountLimits;

        async fn setup() -> Self {
            Self(InMemoryAccountLimits::new(), InMemoryTransactions::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.1
        }

        async fn cleanup(self) {}
    }

    crate::account_limits_conformance_tests!(InMemory);
}
