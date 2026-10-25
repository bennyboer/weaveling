use std::collections::BTreeMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use statistics_core::{CatalogError, ProjectLink, SceneLink, WordCount, WordCountCatalog};
use time::OffsetDateTime;

#[derive(Debug, Clone)]
struct Kept {
    project: ProjectLink,
    words: u64,
    counted_at: OffsetDateTime,
}

#[derive(Debug, Default)]
struct Counts {
    kept: BTreeMap<SceneLink, Kept>,
    gone: BTreeMap<SceneLink, OffsetDateTime>,
}

#[derive(Debug, Default)]
pub struct InMemoryWordCountCatalog {
    counts: RwLock<Counts>,
}

impl InMemoryWordCountCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Counts> {
        self.counts
            .read()
            .expect("word count catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Counts> {
        self.counts
            .write()
            .expect("word count catalog lock poisoned")
    }
}

#[async_trait]
impl WordCountCatalog for InMemoryWordCountCatalog {
    async fn keep(
        &self,
        scene: &SceneLink,
        project: &ProjectLink,
        words: u64,
        counted_at: OffsetDateTime,
    ) -> Result<Option<u64>, CatalogError> {
        let mut counts = self.write();

        if counts.gone.contains_key(scene) {
            return Ok(None);
        }

        let now = Kept {
            project: project.clone(),
            words,
            counted_at,
        };
        let held = counts.kept.entry(scene.clone()).or_insert(now.clone());

        if held.counted_at <= counted_at {
            *held = now;
        }

        Ok(Some(held.words))
    }

    async fn forget_scene(
        &self,
        scene: &SceneLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        let mut counts = self.write();

        counts.gone.entry(scene.clone()).or_insert(at);
        counts.kept.remove(scene);

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        let mut counts = self.write();
        let mut long_gone: Vec<(OffsetDateTime, SceneLink)> = counts
            .gone
            .iter()
            .filter(|(_, laid)| **laid < before)
            .map(|(scene, laid)| (*laid, scene.clone()))
            .collect();
        long_gone.sort();
        long_gone.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));

        for (_, scene) in &long_gone {
            counts.gone.remove(scene);
        }

        Ok(u64::try_from(long_gone.len()).expect("a count of tombstones fits"))
    }

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<WordCount>, CatalogError> {
        Ok(self
            .read()
            .kept
            .iter()
            .filter(|(_, kept)| &kept.project == project)
            .map(|(scene, kept)| WordCount {
                scene: scene.clone(),
                words: kept.words,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;
    use crate::suite::Workbench;

    struct InMemory(InMemoryWordCountCatalog);

    #[async_trait]
    impl Workbench for InMemory {
        type Catalog = InMemoryWordCountCatalog;

        async fn setup() -> Self {
            Self(InMemoryWordCountCatalog::new())
        }

        fn catalog(&self) -> &Self::Catalog {
            &self.0
        }

        async fn cleanup(self) {}
    }

    crate::suite::conformance_tests!(InMemory);
}
