use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use boards_core::{BoardCatalog, BoardId, BoardSummary, CatalogError, IdeaLink, ProjectLink};
use time::OffsetDateTime;

type Listed = HashMap<BoardId, BoardSummary>;

#[derive(Debug, Default)]
struct Pins {
    by_board: HashMap<BoardId, Vec<IdeaLink>>,
    by_idea: HashMap<IdeaLink, BTreeSet<BoardId>>,
    tombstones: HashMap<IdeaLink, OffsetDateTime>,
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

    async fn forget(&self, board: &BoardId) -> Result<(), CatalogError> {
        self.write().remove(board);
        self.holds(*board, &[]).await?;

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

    async fn holds(&self, board: BoardId, ideas: &[IdeaLink]) -> Result<(), CatalogError> {
        let mut pins = self.write_pins();
        let arriving: HashSet<&IdeaLink> = ideas.iter().collect();
        let left_behind = pins
            .by_board
            .insert(board, ideas.to_vec())
            .unwrap_or_default();

        for gone in left_behind.iter().filter(|idea| !arriving.contains(idea)) {
            let Some(holding) = pins.by_idea.get_mut(gone) else {
                continue;
            };
            holding.remove(&board);

            if holding.is_empty() {
                pins.by_idea.remove(gone);
            }
        }

        for held in ideas {
            pins.by_idea.entry(held.clone()).or_default().insert(board);
        }

        Ok(())
    }

    async fn boards_holding(&self, idea: &IdeaLink) -> Result<Vec<BoardId>, CatalogError> {
        Ok(self
            .read_pins()
            .by_idea
            .get(idea)
            .map(|holding| holding.iter().copied().collect())
            .unwrap_or_default())
    }

    async fn lay_tombstone(&self, idea: &IdeaLink, at: OffsetDateTime) -> Result<(), CatalogError> {
        self.write_pins()
            .tombstones
            .entry(idea.clone())
            .or_insert(at);

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        let mut pins = self.write_pins();
        let mut long_gone: Vec<(OffsetDateTime, IdeaLink)> = pins
            .tombstones
            .iter()
            .filter(|(_, laid)| **laid < before)
            .map(|(idea, laid)| (*laid, idea.clone()))
            .collect();
        long_gone.sort_by(|(one_laid, one), (other_laid, other)| {
            one_laid
                .cmp(other_laid)
                .then_with(|| one.as_str().cmp(other.as_str()))
        });
        long_gone.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));

        for (_, idea) in &long_gone {
            pins.tombstones.remove(idea);
        }

        Ok(u64::try_from(long_gone.len()).expect("a count of tombstones fits"))
    }

    async fn held_though_tombstoned(&self, board: BoardId) -> Result<Vec<IdeaLink>, CatalogError> {
        let pins = self.read_pins();
        let mut found: Vec<IdeaLink> = pins
            .by_board
            .get(&board)
            .into_iter()
            .flatten()
            .filter(|held| pins.tombstones.contains_key(held))
            .cloned()
            .collect();
        found.sort_by(|one, other| one.as_str().cmp(other.as_str()));

        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::suite::Workbench;

    struct InMemory(InMemoryBoardCatalog);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryBoardCatalog;

        async fn setup() -> Self {
            Self(InMemoryBoardCatalog::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
