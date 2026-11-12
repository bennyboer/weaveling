use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use assets_core::{CatalogError, OperatorCatalog};
use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use transacting::{Transaction, memory};

type Kept = HashMap<String, (u64, bool)>;

#[derive(Debug, Default)]
pub struct InMemoryOperatorCatalog {
    kept: Arc<Mutex<Kept>>,
}

impl InMemoryOperatorCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn kept(&self) -> MutexGuard<'_, Kept> {
        self.kept
            .lock()
            .expect("the operator catalog is not poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let before = self.kept().clone();
        let into = self.kept.clone();

        memory::undo_on_rollback(transaction, move || {
            *into.lock().expect("the operator catalog is not poisoned") = before;
        })
        .map_err(|failure| CatalogError::Backend(Box::new(failure)))
    }
}

#[async_trait]
impl OperatorCatalog for InMemoryOperatorCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        version: Version,
        operator: bool,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut kept = self.kept();
        let held = kept.entry(account.to_string()).or_insert((0, operator));
        if held.0 < version.count() {
            *held = (version.count(), operator);
        }

        Ok(())
    }

    async fn operates(
        &self,
        _transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<bool, CatalogError> {
        Ok(self
            .kept()
            .get(&account.to_string())
            .is_some_and(|(_, operator)| *operator))
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;
    use crate::operators_suite::Workbench;

    struct InMemory(InMemoryOperatorCatalog, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryOperatorCatalog;

        async fn setup() -> Self {
            Self(InMemoryOperatorCatalog::new(), InMemoryTransactions::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.1
        }

        async fn cleanup(self) {}
    }

    crate::operators_conformance_tests!(InMemory);
}
