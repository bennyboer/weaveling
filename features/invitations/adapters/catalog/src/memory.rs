use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use email::EmailAddress;
use eventsourcing::{AccountId, Version};
use invitations_core::{
    CatalogError, InvitationCatalog, InvitationId, InvitationSummary, InvitationTokenDigest,
    Outcome, Person, ProjectLink, ProjectName,
};
use time::OffsetDateTime;
use transacting::{Failure, Transaction};

type Kept = BTreeMap<InvitationId, InvitationSummary>;

#[derive(Debug, Default)]
pub struct InMemoryInvitationCatalog {
    tombstones: Arc<RwLock<BTreeMap<InvitationId, OffsetDateTime>>>,
    kept: Arc<RwLock<Kept>>,
    people: Arc<RwLock<BTreeMap<String, Person>>>,
    projects: Arc<RwLock<KnownProjects>>,
}

#[derive(Debug, Clone, Default)]
struct KnownProjects {
    names: BTreeMap<String, ProjectName>,
    tombstones: BTreeMap<String, OffsetDateTime>,
    trash: BTreeMap<String, (Version, bool)>,
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

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let tombstones = self
            .tombstones
            .read()
            .expect("invitation catalog lock poisoned")
            .clone();
        let kept = self
            .kept
            .read()
            .expect("invitation catalog lock poisoned")
            .clone();
        let people = self
            .people
            .read()
            .expect("invitation catalog lock poisoned")
            .clone();
        let projects = self
            .projects
            .read()
            .expect("invitation catalog lock poisoned")
            .clone();
        let into_tombstones = self.tombstones.clone();
        let into_kept = self.kept.clone();
        let into_people = self.people.clone();
        let into_projects = self.projects.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_tombstones
                .write()
                .expect("invitation catalog lock poisoned") = tombstones;
            *into_kept.write().expect("invitation catalog lock poisoned") = kept;
            *into_people
                .write()
                .expect("invitation catalog lock poisoned") = people;
            *into_projects
                .write()
                .expect("invitation catalog lock poisoned") = projects;
        })
        .map_err(unreachable)
    }
}

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl InvitationCatalog for InMemoryInvitationCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &InvitationSummary,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let tombstones = self
            .tombstones
            .read()
            .expect("invitation catalog lock poisoned");
        if tombstones.contains_key(&summary.id) {
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

    async fn open_into(&self, project: &ProjectLink) -> Result<Vec<EmailAddress>, CatalogError> {
        let mut addresses: Vec<EmailAddress> = self
            .read()
            .values()
            .filter(|summary| {
                summary.outcome == Outcome::Open && summary.scope.project() == Some(project)
            })
            .map(|summary| summary.to.clone())
            .collect();
        addresses.sort_by(|one, other| one.as_str().cmp(other.as_str()));
        addresses.dedup();

        Ok(addresses)
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

    async fn forget(
        &self,
        transaction: &mut Transaction,
        invitation: &InvitationId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut tombstones = self
            .tombstones
            .write()
            .expect("invitation catalog lock poisoned");
        tombstones.entry(*invitation).or_insert(at);
        self.write().remove(invitation);

        Ok(())
    }

    async fn forget_invitation_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut tombstones = self
            .tombstones
            .write()
            .expect("invitation catalog lock poisoned");
        let mut long_gone: Vec<(OffsetDateTime, InvitationId)> = tombstones
            .iter()
            .filter(|(_, laid)| **laid < before)
            .map(|(invitation, laid)| (*laid, *invitation))
            .collect();
        long_gone.sort();
        long_gone.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));

        for (_, invitation) in &long_gone {
            tombstones.remove(invitation);
        }

        Ok(u64::try_from(long_gone.len()).expect("a count of tombstones fits"))
    }

    async fn remember_person(
        &self,
        transaction: &mut Transaction,
        person: &Person,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.people
            .write()
            .expect("invitation catalog lock poisoned")
            .insert(person.account.to_string(), person.clone());

        Ok(())
    }

    async fn remember_project_name(
        &self,
        transaction: &mut Transaction,
        named: &ProjectName,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
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
        transaction: &mut Transaction,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut projects = self
            .projects
            .write()
            .expect("invitation catalog lock poisoned");
        projects.names.remove(project.as_str());
        projects.trash.remove(project.as_str());
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
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.undo_on_rollback(transaction)?;
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

    async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut projects = self
            .projects
            .write()
            .expect("invitation catalog lock poisoned");
        if projects.tombstones.contains_key(project.as_str()) {
            return Ok(());
        }
        if projects
            .trash
            .get(project.as_str())
            .is_none_or(|(known, _)| *known < version)
        {
            projects
                .trash
                .insert(project.to_string(), (version, in_the_trash));
        }

        Ok(())
    }

    async fn is_in_the_trash(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
        Ok(self
            .projects
            .read()
            .expect("invitation catalog lock poisoned")
            .trash
            .get(project.as_str())
            .is_some_and(|(_, in_the_trash)| *in_the_trash))
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
    use transacting::{InMemoryTransactions, Transactions};

    use crate::suite::Workbench;

    struct InMemory(InMemoryInvitationCatalog, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryInvitationCatalog;

        async fn setup() -> Self {
            Self(
                InMemoryInvitationCatalog::new(),
                InMemoryTransactions::new(),
            )
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
