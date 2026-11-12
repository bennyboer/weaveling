use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use assets_core::{AssetCatalog, AssetId, AssetSummary, CatalogError, Owner, ProjectLink, Usage};
use async_trait::async_trait;
use eventsourcing::AccountId;
use transacting::{Transaction, memory};

type Kept = HashMap<AssetId, AssetSummary>;

#[derive(Debug, Default)]
pub struct InMemoryAssetCatalog {
    kept: Arc<Mutex<Kept>>,
}

impl InMemoryAssetCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn kept(&self) -> MutexGuard<'_, Kept> {
        self.kept.lock().expect("the asset catalog is not poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let before = self.kept().clone();
        let into = self.kept.clone();

        memory::undo_on_rollback(transaction, move || {
            *into.lock().expect("the asset catalog is not poisoned") = before;
        })
        .map_err(|failure| CatalogError::Backend(Box::new(failure)))
    }
}

#[async_trait]
impl AssetCatalog for InMemoryAssetCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &AssetSummary,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.kept().insert(summary.id, summary.clone());

        Ok(())
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        id: &AssetId,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.kept().remove(id);

        Ok(())
    }

    async fn usage_within(
        &self,
        _transaction: &mut Transaction,
        uploader: Option<&AccountId>,
        project: Option<&ProjectLink>,
    ) -> Result<Usage, CatalogError> {
        let kept = self.kept();
        let sum = |counts: &dyn Fn(&AssetSummary) -> bool| {
            kept.values()
                .filter(|summary| counts(summary))
                .map(|summary| summary.total_size)
                .sum::<u64>()
        };

        Ok(Usage {
            instance: sum(&|_| true),
            account: sum(&|summary| uploader.is_some() && summary.uploader.as_ref() == uploader),
            project: sum(
                &|summary| matches!(&summary.owner, Owner::Project(held) if Some(held) == project),
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;
    use crate::suite::Workbench;

    struct InMemory(InMemoryAssetCatalog, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryAssetCatalog;

        async fn setup() -> Self {
            Self(InMemoryAssetCatalog::new(), InMemoryTransactions::new())
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
