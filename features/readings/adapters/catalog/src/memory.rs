use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use readings_core::{Block, CatalogError, SceneLink, SceneTextCatalog};
use time::OffsetDateTime;
use transacting::{Failure, Transaction};

#[derive(Debug, Clone)]
struct Kept {
    blocks: Vec<Block>,
    version: u64,
}

#[derive(Debug, Clone, Default)]
struct Texts {
    kept: BTreeMap<SceneLink, Kept>,
    tombstones: BTreeMap<SceneLink, OffsetDateTime>,
}

#[derive(Debug, Default)]
pub struct InMemorySceneTextCatalog {
    texts: Arc<RwLock<Texts>>,
}

impl InMemorySceneTextCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Texts> {
        self.texts.read().expect("scene text catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Texts> {
        self.texts
            .write()
            .expect("scene text catalog lock poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let texts = self.read().clone();
        let into_texts = self.texts.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_texts
                .write()
                .expect("scene text catalog lock poisoned") = texts;
        })
        .map_err(unreachable)
    }
}

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl SceneTextCatalog for InMemorySceneTextCatalog {
    async fn keep(
        &self,
        transaction: &mut Transaction,
        scene: &SceneLink,
        blocks: &[Block],
        version: u64,
    ) -> Result<bool, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut texts = self.write();

        if texts.tombstones.contains_key(scene)
            || texts
                .kept
                .get(scene)
                .is_some_and(|held| held.version >= version)
        {
            return Ok(false);
        }
        texts.kept.insert(
            scene.clone(),
            Kept {
                blocks: blocks.to_vec(),
                version,
            },
        );

        Ok(true)
    }

    async fn forget_scene(
        &self,
        transaction: &mut Transaction,
        scene: &SceneLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut texts = self.write();

        texts.tombstones.entry(scene.clone()).or_insert(at);
        texts.kept.remove(scene);

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut texts = self.write();
        let mut long_gone: Vec<(OffsetDateTime, SceneLink)> = texts
            .tombstones
            .iter()
            .filter(|(_, laid)| **laid < before)
            .map(|(scene, laid)| (*laid, scene.clone()))
            .collect();
        long_gone.sort();
        long_gone.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));

        for (_, scene) in &long_gone {
            texts.tombstones.remove(scene);
        }

        Ok(long_gone.len() as u64)
    }

    async fn texts_of(
        &self,
        scenes: &[SceneLink],
    ) -> Result<HashMap<SceneLink, Vec<Block>>, CatalogError> {
        let texts = self.read();

        Ok(scenes
            .iter()
            .filter_map(|scene| {
                texts
                    .kept
                    .get(scene)
                    .map(|held| (scene.clone(), held.blocks.clone()))
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;
    use crate::suite::Workbench;

    struct InMemory(InMemorySceneTextCatalog, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Catalog = InMemorySceneTextCatalog;

        async fn setup() -> Self {
            Self(InMemorySceneTextCatalog::new(), InMemoryTransactions::new())
        }

        fn catalog(&self) -> &Self::Catalog {
            &self.0
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.1
        }

        async fn cleanup(self) {}
    }

    crate::suite::conformance_tests!(InMemory);
}
