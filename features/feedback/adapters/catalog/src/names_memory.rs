use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use eventsourcing::AccountId;
use feedback_core::{AccountNameCatalog, CatalogError};
use time::OffsetDateTime;
use transacting::{Failure, Transaction};

#[derive(Debug, Clone, Default)]
struct Names {
    kept: HashMap<AccountId, String>,
    tombstones: HashMap<AccountId, OffsetDateTime>,
}

#[derive(Debug, Default)]
pub struct InMemoryAccountNameCatalog {
    names: Arc<RwLock<Names>>,
}

impl InMemoryAccountNameCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Names> {
        self.names
            .read()
            .expect("account name catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Names> {
        self.names
            .write()
            .expect("account name catalog lock poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let names = self.read().clone();
        let into_names = self.names.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_names
                .write()
                .expect("account name catalog lock poisoned") = names;
        })
        .map_err(unreachable)
    }
}

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl AccountNameCatalog for InMemoryAccountNameCatalog {
    async fn remember_name(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        name: &str,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut names = self.write();

        if !names.tombstones.contains_key(account) {
            names.kept.insert(account.clone(), name.to_owned());
        }

        Ok(())
    }

    async fn forget_name(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut names = self.write();

        names.kept.remove(account);
        names.tombstones.entry(account.clone()).or_insert(at);

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut names = self.write();

        let oldest: BTreeMap<(OffsetDateTime, String), AccountId> = names
            .tombstones
            .iter()
            .filter(|(_, laid)| **laid < before)
            .map(|(account, laid)| ((*laid, account.to_string()), account.clone()))
            .collect();
        let forgotten: Vec<AccountId> = oldest
            .into_values()
            .take(usize::try_from(at_most).unwrap_or(usize::MAX))
            .collect();
        for account in &forgotten {
            names.tombstones.remove(account);
        }

        Ok(forgotten.len() as u64)
    }

    async fn names_of(
        &self,
        accounts: &[AccountId],
    ) -> Result<HashMap<AccountId, String>, CatalogError> {
        let names = self.read();

        Ok(accounts
            .iter()
            .filter_map(|account| {
                names
                    .kept
                    .get(account)
                    .map(|name| (account.clone(), name.clone()))
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;
    use crate::suite::Workbench;

    struct InMemory(InMemoryAccountNameCatalog, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Catalog = InMemoryAccountNameCatalog;

        async fn setup() -> Self {
            Self(
                InMemoryAccountNameCatalog::new(),
                InMemoryTransactions::new(),
            )
        }

        fn catalog(&self) -> &Self::Catalog {
            &self.0
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.1
        }

        async fn cleanup(self) {}
    }

    crate::names_suite::names_conformance_tests!(InMemory);
}
