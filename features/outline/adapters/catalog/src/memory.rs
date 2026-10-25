use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use outline_core::{
    Attachment, CatalogError, OutlineCatalog, OutlineId, OutlineSummary, ProjectLink,
};
use time::OffsetDateTime;

type Listed = HashMap<OutlineId, OutlineSummary>;

#[derive(Debug, Default)]
struct Attachments {
    by_outline: HashMap<OutlineId, Vec<Attachment>>,
    by_attachment: HashMap<Attachment, BTreeSet<OutlineId>>,
    tombstones: HashMap<Attachment, OffsetDateTime>,
}

#[derive(Debug, Default)]
pub struct InMemoryOutlineCatalog {
    listed: RwLock<Listed>,
    attached: RwLock<Attachments>,
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
}

#[async_trait]
impl OutlineCatalog for InMemoryOutlineCatalog {
    async fn remember(&self, summary: &OutlineSummary) -> Result<(), CatalogError> {
        self.write().insert(summary.id, summary.clone());

        Ok(())
    }

    async fn forget(&self, outline: &OutlineId) -> Result<(), CatalogError> {
        self.write().remove(outline);
        self.holds(*outline, &[]).await?;

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
        outline: OutlineId,
        attachments: &[Attachment],
    ) -> Result<(), CatalogError> {
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
        attachment: &Attachment,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.write_attachments()
            .tombstones
            .entry(attachment.clone())
            .or_insert(at);

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
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

        Ok(u64::try_from(long_gone.len()).expect("a count of tombstones fits"))
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
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::suite::Workbench;

    struct InMemory(InMemoryOutlineCatalog);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryOutlineCatalog;

        async fn setup() -> Self {
            Self(InMemoryOutlineCatalog::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
