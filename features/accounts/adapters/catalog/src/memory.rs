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
    trash: Arc<RwLock<BTreeMap<String, (Version, bool)>>>,
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
        let trash = self
            .trash
            .read()
            .expect("account catalog lock poisoned")
            .clone();
        let into_trash = self.trash.clone();
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
            *into_trash.write().expect("account catalog lock poisoned") = trash;
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

    async fn forget(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.write().remove(account);
        let gone = account.to_string();
        self.holdings
            .write()
            .expect("account catalog lock poisoned")
            .retain(|(_, held_by), _| *held_by != gone);

        Ok(())
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

    async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut trash = self.trash.write().expect("account catalog lock poisoned");
        if trash.get(project).is_none_or(|(known, _)| *known < version) {
            trash.insert(project.to_owned(), (version, in_the_trash));
        }

        Ok(())
    }

    async fn forget_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.trash
            .write()
            .expect("account catalog lock poisoned")
            .remove(project);

        Ok(())
    }

    async fn is_in_the_trash(&self, project: &str) -> Result<bool, CatalogError> {
        Ok(self
            .trash
            .read()
            .expect("account catalog lock poisoned")
            .get(project)
            .is_some_and(|(_, in_the_trash)| *in_the_trash))
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
        let trash = self.trash.read().expect("account catalog lock poisoned");
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
                    && !trash
                        .get(project)
                        .is_some_and(|(_, in_the_trash)| *in_the_trash)
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
