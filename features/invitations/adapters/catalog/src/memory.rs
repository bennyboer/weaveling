use std::collections::BTreeMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use invitations_core::{CatalogError, InvitationCatalog, InvitationId, InvitationSummary};
use time::OffsetDateTime;

type Kept = BTreeMap<InvitationId, InvitationSummary>;

#[derive(Debug, Default)]
pub struct InMemoryInvitationCatalog {
    kept: RwLock<Kept>,
}

impl InMemoryInvitationCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Kept> {
        self.kept.read().expect("invitation catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Kept> {
        self.kept.write().expect("invitation catalog lock poisoned")
    }
}

#[async_trait]
impl InvitationCatalog for InMemoryInvitationCatalog {
    async fn remember(&self, summary: &InvitationSummary) -> Result<(), CatalogError> {
        let mut kept = self.write();
        if kept
            .get(&summary.id)
            .is_none_or(|known| known.version < summary.version)
        {
            kept.insert(summary.id, summary.clone());
        }

        Ok(())
    }

    async fn listed_after(
        &self,
        after: Option<&InvitationId>,
        at_most: usize,
    ) -> Result<Vec<InvitationSummary>, CatalogError> {
        Ok(self
            .read()
            .values()
            .rev()
            .filter(|summary| summary.stays_listed())
            .filter(|summary| after.is_none_or(|after| summary.id < *after))
            .take(at_most)
            .cloned()
            .collect())
    }

    async fn forget_due(&self, now: OffsetDateTime) -> Result<u64, CatalogError> {
        let mut kept = self.write();
        let before = kept.len();
        kept.retain(|_, summary| summary.forgotten_after >= now);

        Ok(u64::try_from(before - kept.len()).expect("a count fits"))
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use crate::InMemoryInvitationCatalog;
    use crate::suite::Workbench;

    struct InMemory(InMemoryInvitationCatalog);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryInvitationCatalog;

        async fn setup() -> Self {
            Self(InMemoryInvitationCatalog::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
