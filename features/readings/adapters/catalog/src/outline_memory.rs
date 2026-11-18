use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use readings_core::{BookOutline, CatalogError, OutlineCatalog, OutlinedSection, ProjectLink};
use time::OffsetDateTime;
use transacting::{Failure, Transaction};

#[derive(Debug, Clone, Default)]
struct Book {
    title: Option<(String, u64)>,
    outline: Option<(Vec<OutlinedSection>, u64)>,
}

#[derive(Debug, Clone, Default)]
struct Books {
    kept: BTreeMap<ProjectLink, Book>,
    tombstones: BTreeMap<ProjectLink, OffsetDateTime>,
}

#[derive(Debug, Default)]
pub struct InMemoryOutlineCatalog {
    books: Arc<RwLock<Books>>,
}

impl InMemoryOutlineCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Books> {
        self.books.read().expect("outline catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Books> {
        self.books.write().expect("outline catalog lock poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let books = self.read().clone();
        let into_books = self.books.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_books.write().expect("outline catalog lock poisoned") = books;
        })
        .map_err(unreachable)
    }
}

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

fn is_newer<T>(kept: &Option<(T, u64)>, version: u64) -> bool {
    kept.as_ref().is_none_or(|(_, held)| *held < version)
}

#[async_trait]
impl OutlineCatalog for InMemoryOutlineCatalog {
    async fn keep_outline(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        sections: &[OutlinedSection],
        version: u64,
    ) -> Result<bool, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut books = self.write();
        if books.tombstones.contains_key(project) {
            return Ok(false);
        }
        let book = books.kept.entry(project.clone()).or_default();
        if !is_newer(&book.outline, version) {
            return Ok(false);
        }
        book.outline = Some((sections.to_vec(), version));

        Ok(true)
    }

    async fn keep_title(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        title: &str,
        version: u64,
    ) -> Result<bool, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut books = self.write();
        if books.tombstones.contains_key(project) {
            return Ok(false);
        }
        let book = books.kept.entry(project.clone()).or_default();
        if !is_newer(&book.title, version) {
            return Ok(false);
        }
        book.title = Some((title.to_owned(), version));

        Ok(true)
    }

    async fn forget_book(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut books = self.write();

        books.tombstones.entry(project.clone()).or_insert(at);
        books.kept.remove(project);

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut books = self.write();
        let mut long_gone: Vec<(OffsetDateTime, ProjectLink)> = books
            .tombstones
            .iter()
            .filter(|(_, laid)| **laid < before)
            .map(|(project, laid)| (*laid, project.clone()))
            .collect();
        long_gone.sort();
        long_gone.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));

        for (_, project) in &long_gone {
            books.tombstones.remove(project);
        }

        Ok(long_gone.len() as u64)
    }

    async fn outline_of(&self, project: &ProjectLink) -> Result<Option<BookOutline>, CatalogError> {
        Ok(self.read().kept.get(project).map(|book| BookOutline {
            title: book
                .title
                .as_ref()
                .map(|(title, _)| title.clone())
                .unwrap_or_default(),
            sections: book
                .outline
                .as_ref()
                .map(|(sections, _)| sections.clone())
                .unwrap_or_default(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;
    use crate::suite::Workbench;

    struct InMemory(InMemoryOutlineCatalog, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Catalog = InMemoryOutlineCatalog;

        async fn setup() -> Self {
            Self(InMemoryOutlineCatalog::new(), InMemoryTransactions::new())
        }

        fn catalog(&self) -> &Self::Catalog {
            &self.0
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.1
        }

        async fn cleanup(self) {}
    }

    crate::outline_suite::outline_conformance_tests!(InMemory);
}
