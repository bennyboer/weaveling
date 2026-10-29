use std::collections::BTreeMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use email::EmailAddress;
use eventsourcing::AccountId;
use invitations_core::{
    CatalogError, InvitationCatalog, InvitationId, InvitationSummary, InvitationTokenDigest,
    Outcome, Person, ProjectLink, ProjectName,
};
use time::OffsetDateTime;

type Kept = BTreeMap<InvitationId, InvitationSummary>;

#[derive(Debug, Default)]
pub struct InMemoryInvitationCatalog {
    kept: RwLock<Kept>,
    people: RwLock<BTreeMap<String, Person>>,
    projects: RwLock<KnownProjects>,
}

#[derive(Debug, Default)]
struct KnownProjects {
    names: BTreeMap<String, ProjectName>,
    tombstones: BTreeMap<String, OffsetDateTime>,
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

    async fn open_to(&self, to: &EmailAddress) -> Result<Vec<InvitationSummary>, CatalogError> {
        Ok(self
            .read()
            .values()
            .rev()
            .filter(|summary| summary.to == *to && summary.outcome == Outcome::Open)
            .cloned()
            .collect())
    }

    async fn with_digest(
        &self,
        digest: &InvitationTokenDigest,
    ) -> Result<Option<InvitationSummary>, CatalogError> {
        Ok(self
            .read()
            .values()
            .find(|summary| summary.digest == *digest)
            .cloned())
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

    async fn remember_person(&self, person: &Person) -> Result<(), CatalogError> {
        self.people
            .write()
            .expect("invitation catalog lock poisoned")
            .insert(person.account.to_string(), person.clone());

        Ok(())
    }

    async fn remember_project_name(&self, named: &ProjectName) -> Result<(), CatalogError> {
        let mut projects = self
            .projects
            .write()
            .expect("invitation catalog lock poisoned");
        let project = named.project.to_string();
        if !projects.tombstones.contains_key(&project)
            && projects
                .names
                .get(&project)
                .is_none_or(|before| before.version < named.version)
        {
            projects.names.insert(project, named.clone());
        }

        Ok(())
    }

    async fn lay_project_tombstone(
        &self,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        let mut projects = self
            .projects
            .write()
            .expect("invitation catalog lock poisoned");
        projects.names.remove(project.as_str());
        projects.tombstones.entry(project.to_string()).or_insert(at);

        Ok(())
    }

    async fn has_project_tombstone(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
        Ok(self
            .projects
            .read()
            .expect("invitation catalog lock poisoned")
            .tombstones
            .contains_key(project.as_str()))
    }

    async fn forget_project_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        let mut projects = self
            .projects
            .write()
            .expect("invitation catalog lock poisoned");
        let mut long_gone: Vec<(OffsetDateTime, String)> = projects
            .tombstones
            .iter()
            .filter(|(_, laid)| **laid < before)
            .map(|(project, laid)| (*laid, project.clone()))
            .collect();
        long_gone.sort();
        long_gone.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));

        for (_, project) in &long_gone {
            projects.tombstones.remove(project);
        }

        Ok(u64::try_from(long_gone.len()).expect("a count of tombstones fits"))
    }

    async fn project_names(
        &self,
        projects: &[ProjectLink],
    ) -> Result<Vec<ProjectName>, CatalogError> {
        let known = self
            .projects
            .read()
            .expect("invitation catalog lock poisoned");

        Ok(known
            .names
            .values()
            .filter(|named| projects.contains(&named.project))
            .cloned()
            .collect())
    }

    async fn people(&self, accounts: &[AccountId]) -> Result<Vec<Person>, CatalogError> {
        let known = self
            .people
            .read()
            .expect("invitation catalog lock poisoned");

        Ok(known
            .values()
            .filter(|person| accounts.contains(&person.account))
            .cloned()
            .collect())
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
