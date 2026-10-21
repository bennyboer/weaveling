use std::collections::{BTreeMap, BTreeSet};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use scenes_core::{CatalogError, ProjectLink, SceneCatalog, SceneId, SceneSummary, SceneTitle};
use time::OffsetDateTime;

#[derive(Debug, Clone)]
struct Kept {
    project: ProjectLink,
    title: SceneTitle,
    title_version: u64,
    opening: String,
    opened_at: Option<OffsetDateTime>,
}

impl Kept {
    fn of(project: &ProjectLink) -> Self {
        Self {
            project: project.clone(),
            title: SceneTitle::untitled(),
            title_version: 0,
            opening: String::new(),
            opened_at: None,
        }
    }
}

#[derive(Debug, Default)]
struct Scenes {
    kept: BTreeMap<SceneId, Kept>,
    gone: BTreeSet<SceneId>,
}

#[derive(Debug, Default)]
pub struct InMemorySceneCatalog {
    scenes: RwLock<Scenes>,
}

impl InMemorySceneCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Scenes> {
        self.scenes.read().expect("scene catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Scenes> {
        self.scenes.write().expect("scene catalog lock poisoned")
    }

    fn change(&self, id: SceneId, project: &ProjectLink, changing: impl FnOnce(&mut Kept)) {
        let mut scenes = self.write();

        if scenes.gone.contains(&id) {
            return;
        }

        changing(scenes.kept.entry(id).or_insert_with(|| Kept::of(project)));
    }
}

fn summarised(id: SceneId, kept: &Kept) -> SceneSummary {
    SceneSummary {
        id,
        project: kept.project.clone(),
        title: kept.title.clone(),
        opening: kept.opening.clone(),
    }
}

#[async_trait]
impl SceneCatalog for InMemorySceneCatalog {
    async fn remember(&self, id: SceneId, project: &ProjectLink) -> Result<(), CatalogError> {
        self.change(id, project, |_| {});

        Ok(())
    }

    async fn retitle(
        &self,
        id: SceneId,
        project: &ProjectLink,
        title: &SceneTitle,
        version: u64,
    ) -> Result<(), CatalogError> {
        self.change(id, project, |kept| {
            if kept.title_version < version {
                kept.title = title.clone();
                kept.title_version = version;
            }
        });

        Ok(())
    }

    async fn note_opening(
        &self,
        id: SceneId,
        project: &ProjectLink,
        opening: &str,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.change(id, project, |kept| {
            if kept.opened_at.is_none_or(|before| before <= at) {
                kept.opening = opening.to_owned();
                kept.opened_at = Some(at);
            }
        });

        Ok(())
    }

    async fn forget(&self, id: SceneId) -> Result<(), CatalogError> {
        let mut scenes = self.write();

        scenes.gone.insert(id);
        scenes.kept.remove(&id);

        Ok(())
    }

    async fn summary_of(&self, id: SceneId) -> Result<Option<SceneSummary>, CatalogError> {
        Ok(self.read().kept.get(&id).map(|kept| summarised(id, kept)))
    }

    async fn in_project_after(
        &self,
        project: &ProjectLink,
        after: Option<SceneId>,
        at_most: usize,
    ) -> Result<Vec<SceneSummary>, CatalogError> {
        Ok(self
            .read()
            .kept
            .iter()
            .filter(|(id, kept)| &kept.project == project && after.is_none_or(|last| **id > last))
            .take(at_most)
            .map(|(id, kept)| summarised(*id, kept))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;
    use crate::InMemoryQuotationRestylingTasks;
    use crate::suite::Workbench;

    struct InMemory(InMemorySceneCatalog, InMemoryQuotationRestylingTasks);

    #[async_trait]
    impl Workbench for InMemory {
        type Catalog = InMemorySceneCatalog;
        type Tasks = InMemoryQuotationRestylingTasks;

        async fn setup() -> Self {
            Self(
                InMemorySceneCatalog::new(),
                InMemoryQuotationRestylingTasks::new(),
            )
        }

        fn catalog(&self) -> &Self::Catalog {
            &self.0
        }

        fn tasks(&self) -> &Self::Tasks {
            &self.1
        }

        async fn cleanup(self) {}
    }

    crate::suite::conformance_tests!(InMemory);
}
