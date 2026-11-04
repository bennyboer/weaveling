use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use eventsourcing::AccountId;
use notifications_core::{
    CatalogError, NoticeCatalog, NoticeId, NoticeSummary, Person, ProjectLink, ProjectName,
};
use time::OffsetDateTime;
use transacting::{Failure, Transaction};

#[derive(Debug, Default)]
pub struct InMemoryNoticeCatalog {
    kept: Arc<RwLock<HashMap<NoticeId, NoticeSummary>>>,
    people: Arc<RwLock<HashMap<AccountId, String>>>,
    project_names: Arc<RwLock<HashMap<ProjectLink, ProjectName>>>,
    tombstones: Arc<RwLock<HashMap<ProjectLink, OffsetDateTime>>>,
    forgotten: Arc<RwLock<HashMap<NoticeId, OffsetDateTime>>>,
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

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let kept = self
            .kept
            .read()
            .expect("notice catalog lock poisoned")
            .clone();
        let people = self
            .people
            .read()
            .expect("notice catalog lock poisoned")
            .clone();
        let project_names = self
            .project_names
            .read()
            .expect("notice catalog lock poisoned")
            .clone();
        let tombstones = self
            .tombstones
            .read()
            .expect("notice catalog lock poisoned")
            .clone();
        let into_kept = self.kept.clone();
        let into_people = self.people.clone();
        let into_project_names = self.project_names.clone();
        let forgotten = self
            .forgotten
            .read()
            .expect("notice catalog lock poisoned")
            .clone();
        let into_tombstones = self.tombstones.clone();
        let into_forgotten = self.forgotten.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_kept.write().expect("notice catalog lock poisoned") = kept;
            *into_people.write().expect("notice catalog lock poisoned") = people;
            *into_project_names
                .write()
                .expect("notice catalog lock poisoned") = project_names;
            *into_tombstones
                .write()
                .expect("notice catalog lock poisoned") = tombstones;
            *into_forgotten
                .write()
                .expect("notice catalog lock poisoned") = forgotten;
        })
        .map_err(unreachable)
    }
}

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl NoticeCatalog for InMemoryNoticeCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &NoticeSummary,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        if self
            .forgotten
            .read()
            .expect("notice catalog lock poisoned")
            .contains_key(&summary.id)
        {
            return Ok(());
        }
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
            .filter(|summary| &summary.account == account && summary.dismissed_at.is_none())
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

    async fn remember_person(
        &self,
        transaction: &mut Transaction,
        person: &Person,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
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

    async fn remember_project_name(
        &self,
        transaction: &mut Transaction,
        named: &ProjectName,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
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
        transaction: &mut Transaction,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
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
            .filter(|summary| {
                summary.dismissed_at.is_none() && summary.about.project() == Some(project)
            })
            .map(|summary| summary.account.clone())
            .collect();
        noticed.sort_by(|one, other| one.as_str().cmp(other.as_str()));
        noticed.dedup();

        Ok(noticed)
    }

    async fn due_for_forgetting(
        &self,
        dismissed_before: OffsetDateTime,
        noted_before: OffsetDateTime,
        at_most: usize,
    ) -> Result<Vec<NoticeId>, CatalogError> {
        let mut due: Vec<NoticeId> = self
            .read()
            .values()
            .filter(|summary| match summary.dismissed_at {
                Some(dismissed_at) => dismissed_at < dismissed_before,
                None => summary.noted_at < noted_before,
            } || summary.about.project().is_some_and(|project| self.buried(project)))
            .map(|summary| summary.id)
            .collect();
        due.sort();
        due.truncate(at_most);

        Ok(due)
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        notice: &NoticeId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.forgotten
            .write()
            .expect("notice catalog lock poisoned")
            .entry(*notice)
            .or_insert(at);
        self.write().remove(notice);

        Ok(())
    }

    async fn forget_notice_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut forgotten = self
            .forgotten
            .write()
            .expect("notice catalog lock poisoned");
        let mut old: Vec<(OffsetDateTime, NoticeId)> = forgotten
            .iter()
            .filter(|(_, laid_at)| **laid_at < before)
            .map(|(notice, laid_at)| (*laid_at, *notice))
            .collect();
        old.sort();
        old.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));
        for (_, notice) in &old {
            forgotten.remove(notice);
        }

        Ok(old.len() as u64)
    }

    async fn forget_project_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut tombstones = self
            .tombstones
            .write()
            .expect("notice catalog lock poisoned");
        let mut old: Vec<(OffsetDateTime, ProjectLink)> = tombstones
            .iter()
            .filter(|(_, laid_at)| **laid_at < before)
            .map(|(project, laid_at)| (*laid_at, project.clone()))
            .collect();
        old.sort_by(|one, other| (one.0, one.1.as_str()).cmp(&(other.0, other.1.as_str())));
        old.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));
        for (_, project) in &old {
            tombstones.remove(project);
        }

        Ok(old.len() as u64)
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;

    struct InMemory(InMemoryNoticeCatalog, InMemoryTransactions);

    #[async_trait]
    impl crate::suite::Workbench for InMemory {
        type Store = InMemoryNoticeCatalog;

        async fn setup() -> Self {
            Self(InMemoryNoticeCatalog::new(), InMemoryTransactions::new())
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
