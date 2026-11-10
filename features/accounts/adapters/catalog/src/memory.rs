use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use accounts_core::{AccountCatalog, AccountId, AccountSummary, CatalogError, InProject};
use async_trait::async_trait;
use eventsourcing::Version;
use transacting::{Failure, Transaction};

type Listed = BTreeMap<AccountId, AccountSummary>;

type Holdings = BTreeMap<(String, String), (Version, Option<InProject>)>;

#[derive(Debug, Default)]
pub struct InMemoryAccountCatalog {
    listed: Arc<RwLock<Listed>>,
    holdings: Arc<RwLock<Holdings>>,
    disbanded: Arc<RwLock<BTreeMap<String, Version>>>,
}

impl InMemoryAccountCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Listed> {
        self.listed.read().expect("account catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Listed> {
        self.listed.write().expect("account catalog lock poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let listed = self
            .listed
            .read()
            .expect("account catalog lock poisoned")
            .clone();
        let holdings = self
            .holdings
            .read()
            .expect("account catalog lock poisoned")
            .clone();
        let disbanded = self
            .disbanded
            .read()
            .expect("account catalog lock poisoned")
            .clone();
        let into_listed = self.listed.clone();
        let into_holdings = self.holdings.clone();
        let into_disbanded = self.disbanded.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_listed.write().expect("account catalog lock poisoned") = listed;
            *into_holdings
                .write()
                .expect("account catalog lock poisoned") = holdings;
            *into_disbanded
                .write()
                .expect("account catalog lock poisoned") = disbanded;
        })
        .map_err(unreachable)
    }
}

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl AccountCatalog for InMemoryAccountCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &AccountSummary,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut listed = self.write();
        if listed
            .get(&summary.id)
            .is_none_or(|kept| kept.version < summary.version)
        {
            listed.insert(summary.id, summary.clone());
        }

        Ok(())
    }

    async fn listed_after(
        &self,
        after: Option<&AccountId>,
        at_most: usize,
    ) -> Result<Vec<AccountSummary>, CatalogError> {
        Ok(self
            .read()
            .values()
            .rev()
            .filter(|summary| after.is_none_or(|after| summary.id < *after))
            .take(at_most)
            .cloned()
            .collect())
    }

    async fn remember_holding(
        &self,
        transaction: &mut Transaction,
        project: &str,
        account: &AccountId,
        version: Version,
        held: Option<InProject>,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut holdings = self
            .holdings
            .write()
            .expect("account catalog lock poisoned");
        let key = (project.to_owned(), account.to_string());
        if holdings.get(&key).is_none_or(|(known, _)| *known < version) {
            holdings.insert(key, (version, held));
        }

        Ok(())
    }

    async fn remember_disbanded(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.disbanded
            .write()
            .expect("account catalog lock poisoned")
            .entry(project.to_owned())
            .or_insert(version);

        Ok(())
    }

    async fn projects_to_hand_over(
        &self,
        account: &AccountId,
    ) -> Result<Vec<String>, CatalogError> {
        let holdings = self.holdings.read().expect("account catalog lock poisoned");
        let disbanded = self
            .disbanded
            .read()
            .expect("account catalog lock poisoned");
        let me = account.to_string();
        let mut others: BTreeMap<&str, Vec<InProject>> = BTreeMap::new();
        for ((project, by), (_, held)) in holdings.iter() {
            if let Some(held) = held
                && *by != me
            {
                others.entry(project.as_str()).or_default().push(*held);
            }
        }

        Ok(holdings
            .iter()
            .filter(|((project, by), (_, held))| {
                *by == me
                    && *held == Some(InProject::Owner)
                    && !disbanded.contains_key(project)
                    && others
                        .get(project.as_str())
                        .is_some_and(|others| others.iter().all(|other| *other != InProject::Owner))
            })
            .map(|((project, _), _)| project.clone())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use crate::InMemoryAccountCatalog;
    use transacting::{InMemoryTransactions, Transactions};

    use crate::suite::Workbench;

    struct InMemory(InMemoryAccountCatalog, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryAccountCatalog;

        async fn setup() -> Self {
            Self(InMemoryAccountCatalog::new(), InMemoryTransactions::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.1
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
