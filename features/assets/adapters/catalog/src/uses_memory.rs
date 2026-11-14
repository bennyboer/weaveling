use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use assets_core::{AssetId, AssetUse, AssetUses, CatalogError, Holder, Purpose};
use async_trait::async_trait;
use eventsourcing::Version;
use time::OffsetDateTime;
use transacting::{Transaction, memory};

/// What a holder uses for one purpose: the asset, or none once it stopped, as of a version.
#[derive(Debug, Clone)]
struct Slot {
    version: u64,
    asset: Option<AssetId>,
    gone_at: Option<OffsetDateTime>,
}

type Kept = HashMap<(Purpose, Holder), Slot>;

#[derive(Debug, Default)]
pub struct InMemoryAssetUses {
    kept: Arc<Mutex<Kept>>,
}

impl InMemoryAssetUses {
    pub fn new() -> Self {
        Self::default()
    }

    fn kept(&self) -> MutexGuard<'_, Kept> {
        self.kept.lock().expect("the asset uses are not poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let before = self.kept().clone();
        let into = self.kept.clone();

        memory::undo_on_rollback(transaction, move || {
            *into.lock().expect("the asset uses are not poisoned") = before;
        })
        .map_err(|failure| CatalogError::Backend(Box::new(failure)))
    }

    fn told(&self, purpose: Purpose, by: &Holder, slot: Slot) {
        let mut kept = self.kept();
        let newer = kept
            .get(&(purpose, by.clone()))
            .is_none_or(|held| held.version < slot.version);
        if newer {
            kept.insert((purpose, by.clone()), slot);
        }
    }
}

#[async_trait]
impl AssetUses for InMemoryAssetUses {
    async fn use_as(
        &self,
        transaction: &mut Transaction,
        asset: &AssetId,
        purpose: Purpose,
        by: &Holder,
        version: Version,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.told(
            purpose,
            by,
            Slot {
                version: version.count(),
                asset: Some(*asset),
                gone_at: None,
            },
        );

        Ok(())
    }

    async fn stop_using(
        &self,
        transaction: &mut Transaction,
        purpose: Purpose,
        by: &Holder,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.told(
            purpose,
            by,
            Slot {
                version: version.count(),
                asset: None,
                gone_at: Some(at),
            },
        );

        Ok(())
    }

    async fn uses_of(
        &self,
        _transaction: &mut Transaction,
        assets: &[AssetId],
    ) -> Result<Vec<(AssetId, AssetUse)>, CatalogError> {
        Ok(self
            .kept()
            .iter()
            .filter_map(|((purpose, by), slot)| {
                let asset = slot.asset.filter(|asset| assets.contains(asset))?;
                Some((
                    asset,
                    AssetUse {
                        purpose: *purpose,
                        by: by.clone(),
                    },
                ))
            })
            .collect())
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut kept = self.kept();
        let mut old: Vec<(OffsetDateTime, (Purpose, Holder))> = kept
            .iter()
            .filter_map(|(key, slot)| {
                slot.gone_at
                    .filter(|at| *at < before)
                    .map(|at| (at, key.clone()))
            })
            .collect();
        old.sort_by_key(|(at, _)| *at);
        old.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));
        for (_, key) in &old {
            kept.remove(key);
        }

        Ok(old.len() as u64)
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;
    use crate::uses_suite::Workbench;

    struct InMemory(InMemoryAssetUses, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryAssetUses;

        async fn setup() -> Self {
            Self(InMemoryAssetUses::new(), InMemoryTransactions::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.1
        }

        async fn cleanup(self) {}
    }

    crate::uses_conformance_tests!(InMemory);
}
