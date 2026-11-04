use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use outline_core::{
    Attachment, CatalogError, OutlineCatalog, OutlineId, OutlineSummary, ProjectLink,
};
use time::OffsetDateTime;
use transacting::{Failure, Transaction};

type Listed = HashMap<OutlineId, OutlineSummary>;

#[derive(Debug, Clone, Default)]
struct Attachments {
    by_outline: HashMap<OutlineId, Vec<Attachment>>,
    by_attachment: HashMap<Attachment, BTreeSet<OutlineId>>,
    tombstones: HashMap<Attachment, OffsetDateTime>,
}

#[derive(Debug, Default)]
pub struct InMemoryOutlineCatalog {
    projects_gone: Arc<RwLock<HashMap<ProjectLink, OffsetDateTime>>>,
    forgotten: Arc<RwLock<HashMap<OutlineId, OffsetDateTime>>>,
    listed: Arc<RwLock<Listed>>,
    attached: Arc<RwLock<Attachments>>,
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

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let forgotten = self
            .forgotten
            .read()
            .expect("outline catalog lock poisoned")
            .clone();
        let listed = self
            .listed
            .read()
            .expect("outline catalog lock poisoned")
            .clone();
        let attached = self
            .attached
            .read()
            .expect("outline catalog lock poisoned")
            .clone();
        let into_forgotten = self.forgotten.clone();
        let into_listed = self.listed.clone();
        let into_attached = self.attached.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_forgotten
                .write()
                .expect("outline catalog lock poisoned") = forgotten;
            *into_listed.write().expect("outline catalog lock poisoned") = listed;
            *into_attached
                .write()
                .expect("outline catalog lock poisoned") = attached;
        })
        .map_err(unreachable)
    }
}

impl InMemoryOutlineCatalog {
    fn undo_projects_gone_on_rollback(
        &self,
        transaction: &mut Transaction,
    ) -> Result<(), CatalogError> {
        let projects_gone = self
            .projects_gone
            .read()
            .expect("outline catalog lock poisoned")
            .clone();
        let into_projects_gone = self.projects_gone.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_projects_gone
                .write()
                .expect("outline catalog lock poisoned") = projects_gone;
        })
        .map_err(unreachable)
    }
}

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl OutlineCatalog for InMemoryOutlineCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &OutlineSummary,
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
        outline: &OutlineId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.forgotten
            .write()
            .expect("the catalog is not poisoned")
            .entry(*outline)
            .or_insert(at);
        self.write().remove(outline);
        self.holds(transaction, *outline, &[]).await?;

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

    async fn holds(
        &self,
        transaction: &mut Transaction,
        outline: OutlineId,
        attachments: &[Attachment],
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut attached = self.write_attachments();
        let arriving: HashSet<&Attachment> = attachments.iter().collect();
        let left_behind = attached
            .by_outline
            .insert(outline, attachments.to_vec())
            .unwrap_or_default();

        for gone in left_behind
            .iter()
            .filter(|attachment| !arriving.contains(attachment))
        {
            let Some(holding) = attached.by_attachment.get_mut(gone) else {
                continue;
            };
            holding.remove(&outline);

            if holding.is_empty() {
                attached.by_attachment.remove(gone);
            }
        }

        for held in attachments {
            attached
                .by_attachment
                .entry(held.clone())
                .or_default()
                .insert(outline);
        }

        Ok(())
    }

    async fn outlines_holding(
        &self,
        attachment: &Attachment,
    ) -> Result<Vec<OutlineId>, CatalogError> {
        Ok(self
            .read_attachments()
            .by_attachment
            .get(attachment)
            .map(|holding| holding.iter().copied().collect())
            .unwrap_or_default())
    }

    async fn lay_tombstone(
        &self,
        transaction: &mut Transaction,
        attachment: &Attachment,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.write_attachments()
            .tombstones
            .entry(attachment.clone())
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
        let mut attached = self.write_attachments();
        let mut long_gone: Vec<(OffsetDateTime, Attachment)> = attached
            .tombstones
            .iter()
            .filter(|(_, laid)| **laid < before)
            .map(|(attachment, laid)| (*laid, attachment.clone()))
            .collect();
        long_gone.sort_by(|(one_laid, one), (other_laid, other)| {
            one_laid
                .cmp(other_laid)
                .then_with(|| one.as_str().cmp(other.as_str()))
        });
        long_gone.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));

        for (_, attachment) in &long_gone {
            attached.tombstones.remove(attachment);
        }

        let pieces = u64::try_from(long_gone.len()).expect("a count of tombstones fits");
        drop(attached);

        let summaries = {
            let mut forgotten = self.forgotten.write().expect("the catalog is not poisoned");
            let mut summaries_gone: Vec<(OffsetDateTime, OutlineId)> = forgotten
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
                .expect("outline catalog lock poisoned");
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

    async fn held_though_tombstoned(
        &self,
        outline: OutlineId,
    ) -> Result<Vec<Attachment>, CatalogError> {
        let attached = self.read_attachments();
        let mut found: Vec<Attachment> = attached
            .by_outline
            .get(&outline)
            .into_iter()
            .flatten()
            .filter(|held| attached.tombstones.contains_key(held))
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
            .expect("outline catalog lock poisoned")
            .entry(project.clone())
            .or_insert(at);

        Ok(())
    }

    async fn has_project_tombstone(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
        Ok(self
            .projects_gone
            .read()
            .expect("outline catalog lock poisoned")
            .contains_key(project))
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
    use super::*;

    use transacting::{InMemoryTransactions, Transactions};

    use crate::suite::Workbench;

    struct InMemory(InMemoryOutlineCatalog, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryOutlineCatalog;

        async fn setup() -> Self {
            Self(InMemoryOutlineCatalog::new(), InMemoryTransactions::new())
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
