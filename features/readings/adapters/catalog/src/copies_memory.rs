use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use readings_core::{
    CatalogError, ProjectLink, ReadingCopyCatalog, ReadingCopyId, ReadingCopySummary,
};
use time::OffsetDateTime;
use transacting::{Failure, Transaction};

#[derive(Debug, Default)]
pub struct InMemoryReadingCopyCatalog {
    copies: Arc<RwLock<BTreeMap<ReadingCopyId, ReadingCopySummary>>>,
}

impl InMemoryReadingCopyCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, BTreeMap<ReadingCopyId, ReadingCopySummary>> {
        self.copies
            .read()
            .expect("reading copy catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, BTreeMap<ReadingCopyId, ReadingCopySummary>> {
        self.copies
            .write()
            .expect("reading copy catalog lock poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let copies = self.read().clone();
        let into_copies = self.copies.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_copies
                .write()
                .expect("reading copy catalog lock poisoned") = copies;
        })
        .map_err(unreachable)
    }
}

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl ReadingCopyCatalog for InMemoryReadingCopyCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &ReadingCopySummary,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.write().insert(summary.copy, summary.clone());

        Ok(())
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        copy: &ReadingCopyId,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.write().remove(copy);

        Ok(())
    }

    async fn copies_of(
        &self,
        project: &ProjectLink,
    ) -> Result<Vec<ReadingCopySummary>, CatalogError> {
        let mut copies: Vec<ReadingCopySummary> = self
            .read()
            .values()
            .filter(|summary| &summary.project == project)
            .cloned()
            .collect();
        copies.sort_by_key(|summary| Reverse((summary.made_at, summary.copy)));

        Ok(copies)
    }

    async fn due_for_deletion(
        &self,
        at: OffsetDateTime,
        at_most: u64,
    ) -> Result<Vec<ReadingCopyId>, CatalogError> {
        let mut due: Vec<(OffsetDateTime, ReadingCopyId)> = self
            .read()
            .values()
            .filter_map(|summary| {
                summary
                    .times
                    .deleted_on
                    .filter(|deleted_on| *deleted_on <= at)
                    .map(|deleted_on| (deleted_on, summary.copy))
            })
            .collect();
        due.sort();

        Ok(due
            .into_iter()
            .take(usize::try_from(at_most).unwrap_or(usize::MAX))
            .map(|(_, copy)| copy)
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

    crate::copies_suite::copies_conformance_tests!(InMemory);
}
