use std::collections::BTreeMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use accounts_core::{AccountCatalog, AccountId, AccountSummary, CatalogError};
use async_trait::async_trait;

type Listed = BTreeMap<AccountId, AccountSummary>;

#[derive(Debug, Default)]
pub struct InMemoryAccountCatalog {
    listed: RwLock<Listed>,
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
}

#[async_trait]
impl AccountCatalog for InMemoryAccountCatalog {
    async fn remember(&self, summary: &AccountSummary) -> Result<(), CatalogError> {
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
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use crate::InMemoryAccountCatalog;
    use crate::suite::Workbench;

    struct InMemory(InMemoryAccountCatalog);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryAccountCatalog;

        async fn setup() -> Self {
            Self(InMemoryAccountCatalog::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
