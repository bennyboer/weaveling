use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use boards_core::{BoardCatalog, BoardId, BoardSummary, CatalogError, ProjectLink};

type Listed = HashMap<BoardId, BoardSummary>;

#[derive(Debug, Default)]
pub struct InMemoryBoardCatalog {
    listed: RwLock<Listed>,
}

impl InMemoryBoardCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Listed> {
        self.listed.read().expect("board catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Listed> {
        self.listed.write().expect("board catalog lock poisoned")
    }
}

#[async_trait]
impl BoardCatalog for InMemoryBoardCatalog {
    async fn remember(&self, summary: &BoardSummary) -> Result<(), CatalogError> {
        self.write().insert(summary.id, summary.clone());

        Ok(())
    }

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<BoardSummary>, CatalogError> {
        let mut found: Vec<BoardSummary> = self
            .read()
            .values()
            .filter(|summary| &summary.project == project)
            .cloned()
            .collect();
        found.sort_by_key(|summary| summary.id);

        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    crate::conformance_tests!(InMemoryBoardCatalog::new());
}
