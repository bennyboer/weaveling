use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use feedback_core::{CatalogError, KnownReadingCopy, ReadingCopyCatalog, ReadingCopyLink};
use time::OffsetDateTime;
use transacting::{Failure, Transaction};

#[derive(Debug, Clone)]
struct Kept {
    known: KnownReadingCopy,
    version: u64,
}

#[derive(Debug, Clone, Default)]
struct Copies {
    kept: HashMap<ReadingCopyLink, Kept>,
    tombstones: HashMap<ReadingCopyLink, OffsetDateTime>,
}

#[derive(Debug, Default)]
pub struct InMemoryReadingCopyCatalog {
    copies: Arc<RwLock<Copies>>,
}

impl InMemoryReadingCopyCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Copies> {
        self.copies.read().expect("copy catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Copies> {
        self.copies.write().expect("copy catalog lock poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let copies = self.read().clone();
        let into_copies = self.copies.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_copies.write().expect("copy catalog lock poisoned") = copies;
        })
        .map_err(unreachable)
    }
}

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl ReadingCopyCatalog for InMemoryReadingCopyCatalog {
    async fn keep(
        &self,
        transaction: &mut Transaction,
        copy: &ReadingCopyLink,
        known: &KnownReadingCopy,
        version: u64,
    ) -> Result<bool, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut copies = self.write();

        let newer = !copies.tombstones.contains_key(copy)
            && copies
                .kept
                .get(copy)
                .is_none_or(|kept| kept.version < version);
        if newer {
            copies.kept.insert(
                copy.clone(),
                Kept {
                    known: known.clone(),
                    version,
                },
            );
        }

        Ok(newer)
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        copy: &ReadingCopyLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut copies = self.write();

        copies.kept.remove(copy);
        copies.tombstones.entry(copy.clone()).or_insert(at);

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut copies = self.write();

        let oldest: BTreeMap<(OffsetDateTime, ReadingCopyLink), ()> = copies
            .tombstones
            .iter()
            .filter(|(_, laid)| **laid < before)
            .map(|(copy, laid)| ((*laid, copy.clone()), ()))
            .collect();
        let forgotten: Vec<ReadingCopyLink> = oldest
            .into_keys()
            .map(|(_, copy)| copy)
            .take(usize::try_from(at_most).unwrap_or(usize::MAX))
            .collect();
        for copy in &forgotten {
            copies.tombstones.remove(copy);
        }

        Ok(forgotten.len() as u64)
    }

    async fn known(
        &self,
        asked: &[ReadingCopyLink],
    ) -> Result<HashMap<ReadingCopyLink, KnownReadingCopy>, CatalogError> {
        let copies = self.read();

        Ok(asked
            .iter()
            .filter_map(|copy| {
                copies
                    .kept
                    .get(copy)
                    .map(|kept| (copy.clone(), kept.known.clone()))
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

    struct InMemory(InMemoryReadingCopyCatalog, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Catalog = InMemoryReadingCopyCatalog;

        async fn setup() -> Self {
            Self(
                InMemoryReadingCopyCatalog::new(),
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

    crate::reading_copies_suite::reading_copies_conformance_tests!(InMemory);
}
