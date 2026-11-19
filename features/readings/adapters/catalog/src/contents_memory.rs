use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use readings_core::{CatalogError, Contents, ProjectLink, ReadingCopyContents, ReadingCopyId};
use transacting::{Failure, Transaction};

#[derive(Debug, Default)]
pub struct InMemoryReadingCopyContents {
    contents: Arc<RwLock<HashMap<ReadingCopyId, Contents>>>,
}

impl InMemoryReadingCopyContents {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, HashMap<ReadingCopyId, Contents>> {
        self.contents
            .read()
            .expect("reading copy contents lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, HashMap<ReadingCopyId, Contents>> {
        self.contents
            .write()
            .expect("reading copy contents lock poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let contents = self.read().clone();
        let into_contents = self.contents.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_contents
                .write()
                .expect("reading copy contents lock poisoned") = contents;
        })
        .map_err(unreachable)
    }
}

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl ReadingCopyContents for InMemoryReadingCopyContents {
    async fn keep(
        &self,
        transaction: &mut Transaction,
        copy: &ReadingCopyId,
        contents: &Contents,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.write()
            .entry(*copy)
            .or_insert_with(|| contents.clone());

        Ok(())
    }

    async fn contents_of(&self, copy: &ReadingCopyId) -> Result<Option<Contents>, CatalogError> {
        Ok(self.read().get(copy).cloned())
    }

    async fn copies_of(&self, project: &ProjectLink) -> Result<Vec<ReadingCopyId>, CatalogError> {
        let mut copies: Vec<ReadingCopyId> = self
            .read()
            .iter()
            .filter(|(_, contents)| &contents.project == project)
            .map(|(copy, _)| *copy)
            .collect();
        copies.sort();

        Ok(copies)
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
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;
    use crate::suite::Workbench;

    struct InMemory(InMemoryReadingCopyContents, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Catalog = InMemoryReadingCopyContents;

        async fn setup() -> Self {
            Self(
                InMemoryReadingCopyContents::new(),
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

    crate::contents_suite::contents_conformance_tests!(InMemory);
}
