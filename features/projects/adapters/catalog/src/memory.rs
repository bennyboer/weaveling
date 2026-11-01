use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use projects_core::{CatalogError, ProjectCatalog, ProjectId, ProjectSummary};
use time::OffsetDateTime;

type Listed = HashMap<ProjectId, ProjectSummary>;

#[derive(Debug, Default)]
pub struct InMemoryProjectCatalog {
    forgotten: RwLock<HashMap<ProjectId, OffsetDateTime>>,
    listed: RwLock<Listed>,
}

impl InMemoryProjectCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Listed> {
        self.listed.read().expect("project catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Listed> {
        self.listed.write().expect("project catalog lock poisoned")
    }
}

#[async_trait]
impl ProjectCatalog for InMemoryProjectCatalog {
    async fn remember(&self, summary: &ProjectSummary) -> Result<(), CatalogError> {
        let forgotten = self.forgotten.read().expect("the catalog is not poisoned");
        if forgotten.contains_key(&summary.id) {
            return Ok(());
        }
        let mut listed = self.write();
        if listed
            .get(&summary.id)
            .is_none_or(|known| known.version < summary.version)
        {
            listed.insert(summary.id, summary.clone());
        }

        Ok(())
    }

    async fn forget(&self, id: &ProjectId, at: OffsetDateTime) -> Result<(), CatalogError> {
        self.forgotten
            .write()
            .expect("the catalog is not poisoned")
            .entry(*id)
            .or_insert(at);
        self.write().remove(id);

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        let mut forgotten = self.forgotten.write().expect("the catalog is not poisoned");
        let mut long_gone: Vec<(OffsetDateTime, ProjectId)> = forgotten
            .iter()
            .filter(|(_, laid)| **laid < before)
            .map(|(id, laid)| (*laid, *id))
            .collect();
        long_gone.sort();
        long_gone.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));
        for (_, id) in &long_gone {
            forgotten.remove(id);
        }

        Ok(u64::try_from(long_gone.len()).expect("a count of tombstones fits"))
    }

    async fn all(&self) -> Result<Vec<ProjectSummary>, CatalogError> {
        let mut found: Vec<ProjectSummary> = self.read().values().cloned().collect();
        found.sort_by_key(|summary| std::cmp::Reverse(summary.id));

        Ok(found)
    }

    async fn among(&self, ids: &[ProjectId]) -> Result<Vec<ProjectSummary>, CatalogError> {
        let listed = self.read();
        let mut found: Vec<ProjectSummary> = ids
            .iter()
            .filter_map(|id| listed.get(id))
            .cloned()
            .collect();
        found.sort_by_key(|summary| std::cmp::Reverse(summary.id));
        found.dedup_by_key(|summary| summary.id);

        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::suite::Workbench;

    struct InMemory(InMemoryProjectCatalog);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryProjectCatalog;

        async fn setup() -> Self {
            Self(InMemoryProjectCatalog::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
