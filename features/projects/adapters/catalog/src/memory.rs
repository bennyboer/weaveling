use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use projects_core::{CatalogError, ProjectCatalog, ProjectId, ProjectSummary};

type Listed = HashMap<ProjectId, ProjectSummary>;

#[derive(Debug, Default)]
pub struct InMemoryProjectCatalog {
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
        self.write().insert(summary.id, summary.clone());

        Ok(())
    }

    async fn forget(&self, id: &ProjectId) -> Result<(), CatalogError> {
        self.write().remove(id);

        Ok(())
    }

    async fn all(&self) -> Result<Vec<ProjectSummary>, CatalogError> {
        let mut found: Vec<ProjectSummary> = self.read().values().cloned().collect();
        found.sort_by_key(|summary| std::cmp::Reverse(summary.id));

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
