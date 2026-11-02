use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use boards_core::{BoardCatalog, BoardId, BoardSummary, CatalogError, IdeaLink, ProjectLink};
use time::OffsetDateTime;
use transacting::{Failure, Transaction};

type Listed = HashMap<BoardId, BoardSummary>;

#[derive(Debug, Clone, Default)]
struct Pins {
    by_board: HashMap<BoardId, Vec<IdeaLink>>,
    by_idea: HashMap<IdeaLink, BTreeSet<BoardId>>,
    tombstones: HashMap<IdeaLink, OffsetDateTime>,
}

#[derive(Debug, Default)]
pub struct InMemoryBoardCatalog {
    projects_gone: Arc<RwLock<HashMap<ProjectLink, OffsetDateTime>>>,
    forgotten: Arc<RwLock<HashMap<BoardId, OffsetDateTime>>>,
    listed: Arc<RwLock<Listed>>,
    pins: Arc<RwLock<Pins>>,
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

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let forgotten = self
            .forgotten
            .read()
            .expect("board catalog lock poisoned")
            .clone();
        let listed = self
            .listed
            .read()
            .expect("board catalog lock poisoned")
            .clone();
        let pins = self
            .pins
            .read()
            .expect("board catalog lock poisoned")
            .clone();
        let into_forgotten = self.forgotten.clone();
        let into_listed = self.listed.clone();
        let into_pins = self.pins.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_forgotten.write().expect("board catalog lock poisoned") = forgotten;
            *into_listed.write().expect("board catalog lock poisoned") = listed;
            *into_pins.write().expect("board catalog lock poisoned") = pins;
        })
        .map_err(unreachable)
    }
}

impl InMemoryBoardCatalog {
    fn undo_projects_gone_on_rollback(
        &self,
        transaction: &mut Transaction,
    ) -> Result<(), CatalogError> {
        let projects_gone = self
            .projects_gone
            .read()
            .expect("board catalog lock poisoned")
            .clone();
        let into_projects_gone = self.projects_gone.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_projects_gone
                .write()
                .expect("board catalog lock poisoned") = projects_gone;
        })
        .map_err(unreachable)
    }
}

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl BoardCatalog for InMemoryBoardCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &BoardSummary,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let forgotten = self.forgotten.read().expect("the catalog is not poisoned");
        if forgotten.contains_key(&summary.id) {
            return Ok(());
        }
        self.write().insert(summary.id, summary.clone());

        Ok(())
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        board: &BoardId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.forgotten
            .write()
            .expect("the catalog is not poisoned")
            .entry(*board)
            .or_insert(at);
        self.write().remove(board);
        self.holds(transaction, *board, &[]).await?;

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

    async fn holds(
        &self,
        transaction: &mut Transaction,
        board: BoardId,
        ideas: &[IdeaLink],
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
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

    async fn lay_tombstone(
        &self,
        transaction: &mut Transaction,
        idea: &IdeaLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.write_pins()
            .tombstones
            .entry(idea.clone())
            .or_insert(at);

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.undo_on_rollback(transaction)?;
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

        let pieces = u64::try_from(long_gone.len()).expect("a count of tombstones fits");
        drop(pins);

        let summaries = {
            let mut forgotten = self.forgotten.write().expect("the catalog is not poisoned");
            let mut summaries_gone: Vec<(OffsetDateTime, BoardId)> = forgotten
                .iter()
                .filter(|(_, laid)| **laid < before)
                .map(|(id, laid)| (*laid, *id))
                .collect();
            summaries_gone.sort();
            summaries_gone.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));
            for (_, id) in &summaries_gone {
                forgotten.remove(id);
            }
            u64::try_from(summaries_gone.len()).expect("a count of tombstones fits")
        };

        self.undo_projects_gone_on_rollback(transaction)?;
        let projects = {
            let mut projects_gone = self
                .projects_gone
                .write()
                .expect("board catalog lock poisoned");
            let mut long_gone: Vec<(OffsetDateTime, ProjectLink)> = projects_gone
                .iter()
                .filter(|(_, laid)| **laid < before)
                .map(|(project, laid)| (*laid, project.clone()))
                .collect();
            long_gone.sort_by(|(one_laid, one), (other_laid, other)| {
                one_laid
                    .cmp(other_laid)
                    .then_with(|| one.as_str().cmp(other.as_str()))
            });
            long_gone.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));
            for (_, project) in &long_gone {
                projects_gone.remove(project);
            }
            u64::try_from(long_gone.len()).expect("a count of tombstones fits")
        };

        Ok(pieces + summaries + projects)
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

    async fn lay_project_tombstone(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_projects_gone_on_rollback(transaction)?;
        self.projects_gone
            .write()
            .expect("board catalog lock poisoned")
            .entry(project.clone())
            .or_insert(at);

        Ok(())
    }

    async fn has_project_tombstone(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
        Ok(self
            .projects_gone
            .read()
            .expect("board catalog lock poisoned")
            .contains_key(project))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use transacting::{InMemoryTransactions, Transactions};

    use crate::suite::Workbench;

    struct InMemory(InMemoryBoardCatalog, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryBoardCatalog;

        async fn setup() -> Self {
            Self(InMemoryBoardCatalog::new(), InMemoryTransactions::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.1
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
