use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use boards_core::{BoardCatalog, BoardId, BoardSummary, CatalogError, PieceLink, ProjectLink};

type Listed = HashMap<BoardId, BoardSummary>;

#[derive(Debug, Default)]
struct Pins {
    by_board: HashMap<BoardId, Vec<PieceLink>>,
    by_piece: HashMap<PieceLink, BTreeSet<BoardId>>,
}

#[derive(Debug, Default)]
pub struct InMemoryBoardCatalog {
    listed: RwLock<Listed>,
    pins: RwLock<Pins>,
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

    fn read_pins(&self) -> RwLockReadGuard<'_, Pins> {
        self.pins.read().expect("board catalog lock poisoned")
    }

    fn write_pins(&self) -> RwLockWriteGuard<'_, Pins> {
        self.pins.write().expect("board catalog lock poisoned")
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

    async fn holds(&self, board: BoardId, pieces: &[PieceLink]) -> Result<(), CatalogError> {
        let mut pins = self.write_pins();
        let arriving: HashSet<&PieceLink> = pieces.iter().collect();
        let left_behind = pins
            .by_board
            .insert(board, pieces.to_vec())
            .unwrap_or_default();

        for gone in left_behind.iter().filter(|piece| !arriving.contains(piece)) {
            let Some(holding) = pins.by_piece.get_mut(gone) else {
                continue;
            };
            holding.remove(&board);

            if holding.is_empty() {
                pins.by_piece.remove(gone);
            }
        }

        for held in pieces {
            pins.by_piece.entry(held.clone()).or_default().insert(board);
        }

        Ok(())
    }

    async fn boards_holding(&self, piece: &PieceLink) -> Result<Vec<BoardId>, CatalogError> {
        Ok(self
            .read_pins()
            .by_piece
            .get(piece)
            .map(|holding| holding.iter().copied().collect())
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    crate::conformance_tests!(InMemoryBoardCatalog::new());
}
