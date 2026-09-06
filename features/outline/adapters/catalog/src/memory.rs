use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use outline_core::{
    CatalogError, OutlineCatalog, OutlineId, OutlineSummary, PieceLink, ProjectLink,
};

type Listed = HashMap<OutlineId, OutlineSummary>;

#[derive(Debug, Default)]
struct Attachments {
    by_outline: HashMap<OutlineId, Vec<PieceLink>>,
    by_piece: HashMap<PieceLink, BTreeSet<OutlineId>>,
}

#[derive(Debug, Default)]
pub struct InMemoryOutlineCatalog {
    listed: RwLock<Listed>,
    attached: RwLock<Attachments>,
}

impl InMemoryOutlineCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Listed> {
        self.listed.read().expect("outline catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Listed> {
        self.listed.write().expect("outline catalog lock poisoned")
    }

    fn read_attachments(&self) -> RwLockReadGuard<'_, Attachments> {
        self.attached.read().expect("outline catalog lock poisoned")
    }

    fn write_attachments(&self) -> RwLockWriteGuard<'_, Attachments> {
        self.attached
            .write()
            .expect("outline catalog lock poisoned")
    }
}

#[async_trait]
impl OutlineCatalog for InMemoryOutlineCatalog {
    async fn remember(&self, summary: &OutlineSummary) -> Result<(), CatalogError> {
        self.write().insert(summary.id, summary.clone());

        Ok(())
    }

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<OutlineSummary>, CatalogError> {
        let mut found: Vec<OutlineSummary> = self
            .read()
            .values()
            .filter(|summary| &summary.project == project)
            .cloned()
            .collect();
        found.sort_by_key(|summary| summary.id);

        Ok(found)
    }

    async fn holds(&self, outline: OutlineId, pieces: &[PieceLink]) -> Result<(), CatalogError> {
        let mut attached = self.write_attachments();
        let arriving: HashSet<&PieceLink> = pieces.iter().collect();
        let left_behind = attached
            .by_outline
            .insert(outline, pieces.to_vec())
            .unwrap_or_default();

        for gone in left_behind.iter().filter(|piece| !arriving.contains(piece)) {
            let Some(holding) = attached.by_piece.get_mut(gone) else {
                continue;
            };
            holding.remove(&outline);

            if holding.is_empty() {
                attached.by_piece.remove(gone);
            }
        }

        for held in pieces {
            attached
                .by_piece
                .entry(held.clone())
                .or_default()
                .insert(outline);
        }

        Ok(())
    }

    async fn outlines_holding(&self, piece: &PieceLink) -> Result<Vec<OutlineId>, CatalogError> {
        Ok(self
            .read_attachments()
            .by_piece
            .get(piece)
            .map(|holding| holding.iter().copied().collect())
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    crate::conformance_tests!(InMemoryOutlineCatalog::new());
}
