use std::collections::BTreeMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use invitations_core::{
    CatalogError, InvitationCatalog, InvitationId, InvitationSummary, ProjectLink,
};
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

    fn listed(
        &self,
        project: Option<&ProjectLink>,
        after: Option<&InvitationId>,
        at_most: usize,
    ) -> Vec<InvitationSummary> {
        self.read()
            .values()
            .rev()
            .filter(|summary| summary.stays_listed())
            .filter(|summary| summary.scope.project() == project)
            .filter(|summary| after.is_none_or(|after| summary.id < *after))
            .take(at_most)
            .cloned()
            .collect()
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
        Ok(self.listed(None, after, at_most))
    }

    async fn in_project_after(
        &self,
        project: &ProjectLink,
        after: Option<&InvitationId>,
        at_most: usize,
    ) -> Result<Vec<InvitationSummary>, CatalogError> {
        Ok(self.listed(Some(project), after, at_most))
    }

    async fn due_for_retiring(
        &self,
        now: OffsetDateTime,
        at_most: usize,
    ) -> Result<Vec<InvitationId>, CatalogError> {
        let kept = self.read();
        let mut due: Vec<&InvitationSummary> = kept
            .values()
            .filter(|summary| summary.retire_after < now)
            .collect();
        due.sort_by_key(|summary| summary.retire_after);

        Ok(due
            .into_iter()
            .take(at_most)
            .map(|summary| summary.id)
            .collect())
    }

    async fn forget(&self, invitation: &InvitationId) -> Result<(), CatalogError> {
        self.write().remove(invitation);

        Ok(())
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
