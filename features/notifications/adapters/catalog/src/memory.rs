use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use eventsourcing::AccountId;
use notifications_core::{
    CatalogError, NoticeCatalog, NoticeId, NoticeSummary, Person, ProjectLink, ProjectName,
};
use time::OffsetDateTime;

#[derive(Debug, Default)]
pub struct InMemoryNoticeCatalog {
    kept: RwLock<HashMap<NoticeId, NoticeSummary>>,
    people: RwLock<HashMap<AccountId, String>>,
    project_names: RwLock<HashMap<ProjectLink, ProjectName>>,
    tombstones: RwLock<HashMap<ProjectLink, OffsetDateTime>>,
}

impl InMemoryNoticeCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn buried(&self, project: &ProjectLink) -> bool {
        self.tombstones
            .read()
            .expect("notice catalog lock poisoned")
            .contains_key(project)
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
            .filter(|summary| {
                summary
                    .about
                    .project()
                    .is_none_or(|project| !self.buried(project))
            })
            .cloned()
            .collect();
        open.sort_by_key(|summary| Reverse((summary.noted_at, summary.id)));

        Ok(open)
    }

    async fn remember_person(&self, person: &Person) -> Result<(), CatalogError> {
        self.people
            .write()
            .expect("notice catalog lock poisoned")
            .insert(person.account.clone(), person.name.clone());

        Ok(())
    }

    async fn people(&self, accounts: &[AccountId]) -> Result<Vec<Person>, CatalogError> {
        let known = self.people.read().expect("notice catalog lock poisoned");

        Ok(accounts
            .iter()
            .filter_map(|account| {
                known.get(account).map(|name| Person {
                    account: account.clone(),
                    name: name.clone(),
                })
            })
            .collect())
    }

    async fn remember_project_name(&self, named: &ProjectName) -> Result<(), CatalogError> {
        if self.buried(&named.project) {
            return Ok(());
        }
        let mut known = self
            .project_names
            .write()
            .expect("notice catalog lock poisoned");
        if known
            .get(&named.project)
            .is_none_or(|already| already.version < named.version)
        {
            known.insert(named.project.clone(), named.clone());
        }

        Ok(())
    }

    async fn project_names(
        &self,
        projects: &[ProjectLink],
    ) -> Result<Vec<ProjectName>, CatalogError> {
        let known = self
            .project_names
            .read()
            .expect("notice catalog lock poisoned");

        Ok(projects
            .iter()
            .filter_map(|project| known.get(project).cloned())
            .collect())
    }

    async fn lay_project_tombstone(
        &self,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.tombstones
            .write()
            .expect("notice catalog lock poisoned")
            .entry(project.clone())
            .or_insert(at);
        self.project_names
            .write()
            .expect("notice catalog lock poisoned")
            .remove(project);

        Ok(())
    }

    async fn has_project_tombstone(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
        Ok(self.buried(project))
    }

    async fn accounts_noticed_about(
        &self,
        project: &ProjectLink,
    ) -> Result<Vec<AccountId>, CatalogError> {
        let mut noticed: Vec<AccountId> = self
            .read()
            .values()
            .filter(|summary| !summary.dismissed && summary.about.project() == Some(project))
            .map(|summary| summary.account.clone())
            .collect();
        noticed.sort_by(|one, other| one.as_str().cmp(other.as_str()));
        noticed.dedup();

        Ok(noticed)
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
