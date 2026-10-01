use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use ideas_core::{CatalogError, IdeaCatalog, IdeaId, IdeaSummary, ProjectLink};

type Listed = HashMap<IdeaId, IdeaSummary>;

#[derive(Debug, Default)]
pub struct InMemoryIdeaCatalog {
    listed: RwLock<Listed>,
}

impl InMemoryIdeaCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Listed> {
        self.listed.read().expect("idea catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Listed> {
        self.listed.write().expect("idea catalog lock poisoned")
    }
}

#[async_trait]
impl IdeaCatalog for InMemoryIdeaCatalog {
    async fn remember(&self, summary: &IdeaSummary) -> Result<(), CatalogError> {
        self.write().insert(summary.id, summary.clone());

        Ok(())
    }

    async fn forget(&self, id: &IdeaId) -> Result<(), CatalogError> {
        self.write().remove(id);

        Ok(())
    }

    async fn in_project_after(
        &self,
        project: &ProjectLink,
        after: Option<IdeaId>,
        at_most: usize,
    ) -> Result<Vec<IdeaSummary>, CatalogError> {
        let mut found: Vec<IdeaSummary> = self
            .read()
            .values()
            .filter(|summary| &summary.project == project)
            .filter(|summary| after.is_none_or(|last| summary.id > last))
            .cloned()
            .collect();
        found.sort_by_key(|summary| summary.id);
        found.truncate(at_most);

        Ok(found)
    }

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<IdeaSummary>, CatalogError> {
        let mut found: Vec<IdeaSummary> = self
            .read()
            .values()
            .filter(|summary| &summary.project == project)
            .cloned()
            .collect();
        found.sort_by_key(|summary| std::cmp::Reverse(summary.id));

        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::suite::Workbench;

    struct InMemory(InMemoryIdeaCatalog);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryIdeaCatalog;

        async fn setup() -> Self {
            Self(InMemoryIdeaCatalog::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
