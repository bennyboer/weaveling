use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use pieces_core::{CatalogError, PieceCatalog, PieceId, PieceSummary, ProjectLink};

type Listed = HashMap<PieceId, PieceSummary>;

#[derive(Debug, Default)]
pub struct InMemoryPieceCatalog {
    listed: RwLock<Listed>,
}

impl InMemoryPieceCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Listed> {
        self.listed.read().expect("piece catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Listed> {
        self.listed.write().expect("piece catalog lock poisoned")
    }
}

#[async_trait]
impl PieceCatalog for InMemoryPieceCatalog {
    async fn remember(&self, summary: &PieceSummary) -> Result<(), CatalogError> {
        self.write().insert(summary.id, summary.clone());

        Ok(())
    }

    async fn forget(&self, id: &PieceId) -> Result<(), CatalogError> {
        self.write().remove(id);

        Ok(())
    }

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<PieceSummary>, CatalogError> {
        let mut found: Vec<PieceSummary> = self
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

    crate::conformance_tests!(InMemoryPieceCatalog::new());
}
