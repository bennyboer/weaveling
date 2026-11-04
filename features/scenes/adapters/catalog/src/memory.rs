use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use scenes_core::{CatalogError, ProjectLink, SceneCatalog, SceneId, SceneSummary, SceneTitle};
use time::OffsetDateTime;
use transacting::{Failure, Transaction};

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

#[derive(Debug, Clone, Default)]
struct Scenes {
    kept: BTreeMap<SceneId, Kept>,
    tombstones: BTreeMap<SceneId, OffsetDateTime>,
    project_tombstones: BTreeMap<ProjectLink, OffsetDateTime>,
}

#[derive(Debug, Default)]
pub struct InMemorySceneCatalog {
    scenes: Arc<RwLock<Scenes>>,
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

        if scenes.tombstones.contains_key(&id) {
            return;
        }

        changing(scenes.kept.entry(id).or_insert_with(|| Kept::of(project)));
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let scenes = self
            .scenes
            .read()
            .expect("scene catalog lock poisoned")
            .clone();
        let into_scenes = self.scenes.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_scenes.write().expect("scene catalog lock poisoned") = scenes;
        })
        .map_err(unreachable)
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

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl SceneCatalog for InMemorySceneCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        project: &ProjectLink,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.change(id, project, |_| {});

        Ok(())
    }

    async fn retitle(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        project: &ProjectLink,
        title: &SceneTitle,
        version: u64,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
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
        transaction: &mut Transaction,
        id: SceneId,
        project: &ProjectLink,
        opening: &str,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.change(id, project, |kept| {
            if kept.opened_at.is_none_or(|before| before <= at) {
                kept.opening = opening.to_owned();
                kept.opened_at = Some(at);
            }
        });

        Ok(())
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut scenes = self.write();

        scenes.tombstones.entry(id).or_insert(at);
        scenes.kept.remove(&id);

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut scenes = self.write();
        let mut long_gone: Vec<(OffsetDateTime, SceneId)> = scenes
            .tombstones
            .iter()
            .filter(|(_, laid)| **laid < before)
            .map(|(scene, laid)| (*laid, *scene))
            .collect();
        long_gone.sort();
        long_gone.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));
        let mut projects_long_gone: Vec<(OffsetDateTime, ProjectLink)> = scenes
            .project_tombstones
            .iter()
            .filter(|(_, laid)| **laid < before)
            .map(|(project, laid)| (*laid, project.clone()))
            .collect();
        projects_long_gone.sort();
        projects_long_gone.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));

        for (_, scene) in &long_gone {
            scenes.tombstones.remove(scene);
        }
        for (_, project) in &projects_long_gone {
            scenes.project_tombstones.remove(project);
        }

        Ok(u64::try_from(long_gone.len() + projects_long_gone.len())
            .expect("a count of tombstones fits"))
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

    async fn lay_project_tombstone(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.write()
            .project_tombstones
            .entry(project.clone())
            .or_insert(at);

        Ok(())
    }

    async fn has_project_tombstone(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
        Ok(self.read().project_tombstones.contains_key(project))
    }

    async fn has_project_tombstone_within(
        &self,
        _transaction: &mut Transaction,
        project: &ProjectLink,
    ) -> Result<bool, CatalogError> {
        self.has_project_tombstone(project).await
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;
    use crate::InMemoryQuotationRestylingTasks;
    use crate::suite::Workbench;

    struct InMemory(
        InMemorySceneCatalog,
        InMemoryQuotationRestylingTasks,
        InMemoryTransactions,
    );

    #[async_trait]
    impl Workbench for InMemory {
        type Catalog = InMemorySceneCatalog;
        type Tasks = InMemoryQuotationRestylingTasks;

        async fn setup() -> Self {
            Self(
                InMemorySceneCatalog::new(),
                InMemoryQuotationRestylingTasks::new(),
                InMemoryTransactions::new(),
            )
        }

        fn catalog(&self) -> &Self::Catalog {
            &self.0
        }

        fn tasks(&self) -> &Self::Tasks {
            &self.1
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.2
        }

        async fn cleanup(self) {}
    }

    crate::suite::conformance_tests!(InMemory);
}
