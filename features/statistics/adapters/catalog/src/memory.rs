use std::collections::{BTreeMap, BTreeSet};
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
    gone: BTreeSet<SceneLink>,
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
    ) -> Result<(), CatalogError> {
        let mut counts = self.write();

        if counts.gone.contains(scene) {
            return Ok(());
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

        Ok(())
    }

    async fn forget_scene(&self, scene: &SceneLink) -> Result<(), CatalogError> {
        let mut counts = self.write();

        counts.gone.insert(scene.clone());
        counts.kept.remove(scene);

        Ok(())
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
