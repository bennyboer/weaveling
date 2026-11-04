use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use ideas_core::{CatalogError, IdeaCatalog, IdeaId, IdeaSummary, ProjectLink};
use time::OffsetDateTime;
use transacting::{Failure, Transaction};

type Listed = HashMap<IdeaId, IdeaSummary>;

#[derive(Debug, Default)]
pub struct InMemoryIdeaCatalog {
    forgotten: Arc<RwLock<HashMap<IdeaId, OffsetDateTime>>>,
    listed: Arc<RwLock<Listed>>,
    projects_gone: Arc<RwLock<HashMap<ProjectLink, OffsetDateTime>>>,
}

impl InMemoryIdeaCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Listed> {
        self.listed.read().expect("idea catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Listed> {
        self.listed.write().expect("idea catalog lock poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let forgotten = self
            .forgotten
            .read()
            .expect("idea catalog lock poisoned")
            .clone();
        let listed = self
            .listed
            .read()
            .expect("idea catalog lock poisoned")
            .clone();
        let projects_gone = self
            .projects_gone
            .read()
            .expect("idea catalog lock poisoned")
            .clone();
        let into_forgotten = self.forgotten.clone();
        let into_listed = self.listed.clone();
        let into_projects_gone = self.projects_gone.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_forgotten.write().expect("idea catalog lock poisoned") = forgotten;
            *into_listed.write().expect("idea catalog lock poisoned") = listed;
            *into_projects_gone
                .write()
                .expect("idea catalog lock poisoned") = projects_gone;
        })
        .map_err(unreachable)
    }
}

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl IdeaCatalog for InMemoryIdeaCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &IdeaSummary,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let forgotten = self.forgotten.read().expect("the catalog is not poisoned");
        if forgotten.contains_key(&summary.id) {
            return Ok(());
        }
        let mut listed = self.write();
        if listed
            .get(&summary.id)
            .is_none_or(|known| known.version < summary.version)
        {
            listed.insert(summary.id, summary.clone());
        }

        Ok(())
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        id: &IdeaId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.forgotten
            .write()
            .expect("the catalog is not poisoned")
            .entry(*id)
            .or_insert(at);
        self.write().remove(id);

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut forgotten = self.forgotten.write().expect("the catalog is not poisoned");
        let mut long_gone: Vec<(OffsetDateTime, IdeaId)> = forgotten
            .iter()
            .filter(|(_, laid)| **laid < before)
            .map(|(id, laid)| (*laid, *id))
            .collect();
        long_gone.sort();
        long_gone.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));
        for (_, id) in &long_gone {
            forgotten.remove(id);
        }
        let mut projects_gone = self
            .projects_gone
            .write()
            .expect("idea catalog lock poisoned");
        let mut projects_long_gone: Vec<(OffsetDateTime, String)> = projects_gone
            .iter()
            .filter(|(_, laid)| **laid < before)
            .map(|(project, laid)| (*laid, project.as_str().to_owned()))
            .collect();
        projects_long_gone.sort();
        projects_long_gone.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));
        for (_, project) in &projects_long_gone {
            projects_gone.remove(&ProjectLink::from(project.as_str()));
        }

        Ok(u64::try_from(long_gone.len() + projects_long_gone.len())
            .expect("a count of tombstones fits"))
    }

    async fn in_project_after(
        &self,
        project: &ProjectLink,
        after: Option<IdeaId>,
        at_most: usize,
    ) -> Result<Vec<IdeaSummary>, CatalogError> {
        let mut found: Vec<IdeaSummary> = self
            .read()
            .values()
            .filter(|summary| &summary.project == project)
            .filter(|summary| after.is_none_or(|last| summary.id > last))
            .cloned()
            .collect();
        found.sort_by_key(|summary| summary.id);
        found.truncate(at_most);

        Ok(found)
    }

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<IdeaSummary>, CatalogError> {
        let mut found: Vec<IdeaSummary> = self
            .read()
            .values()
            .filter(|summary| &summary.project == project)
            .cloned()
            .collect();
        found.sort_by_key(|summary| std::cmp::Reverse(summary.id));

        Ok(found)
    }

    async fn lay_project_tombstone(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.projects_gone
            .write()
            .expect("idea catalog lock poisoned")
            .entry(project.clone())
            .or_insert(at);

        Ok(())
    }

    async fn has_project_tombstone_within(
        &self,
        _transaction: &mut Transaction,
        project: &ProjectLink,
    ) -> Result<bool, CatalogError> {
        Ok(self
            .projects_gone
            .read()
            .expect("idea catalog lock poisoned")
            .contains_key(project))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use transacting::{InMemoryTransactions, Transactions};

    use crate::suite::Workbench;

    struct InMemory(InMemoryIdeaCatalog, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryIdeaCatalog;

        async fn setup() -> Self {
            Self(InMemoryIdeaCatalog::new(), InMemoryTransactions::new())
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
