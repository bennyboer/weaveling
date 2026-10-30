use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use eventsourcing::AccountId;
use notifications_core::{CatalogError, NoticeCatalog, NoticeId, NoticeSummary};

#[derive(Debug, Default)]
pub struct InMemoryNoticeCatalog {
    kept: RwLock<HashMap<NoticeId, NoticeSummary>>,
}

impl InMemoryNoticeCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, HashMap<NoticeId, NoticeSummary>> {
        self.kept.read().expect("notice catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, HashMap<NoticeId, NoticeSummary>> {
        self.kept.write().expect("notice catalog lock poisoned")
    }
}

#[async_trait]
impl NoticeCatalog for InMemoryNoticeCatalog {
    async fn remember(&self, summary: &NoticeSummary) -> Result<(), CatalogError> {
        let mut kept = self.write();
        if kept
            .get(&summary.id)
            .is_none_or(|known| known.version < summary.version)
        {
            kept.insert(summary.id, summary.clone());
        }

        Ok(())
    }

    async fn open_for(&self, account: &AccountId) -> Result<Vec<NoticeSummary>, CatalogError> {
        let mut open: Vec<NoticeSummary> = self
            .read()
            .values()
            .filter(|summary| &summary.account == account && !summary.dismissed)
            .cloned()
            .collect();
        open.sort_by_key(|summary| Reverse((summary.noted_at, summary.id)));

        Ok(open)
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;

    struct InMemory(InMemoryNoticeCatalog);

    #[async_trait]
    impl crate::suite::Workbench for InMemory {
        type Store = InMemoryNoticeCatalog;

        async fn setup() -> Self {
            Self(InMemoryNoticeCatalog::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
