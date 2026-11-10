use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use members_core::{
    CatalogError, Leaving, Membership, MembershipCatalog, Person, ProjectLink, ProjectMembers, Role,
};
use time::OffsetDateTime;
use transacting::{Failure, Transaction};

type Kept = BTreeMap<String, ProjectMembers>;

type Trash = BTreeMap<String, (Version, bool)>;

type LeavingAccounts = BTreeMap<String, (Version, Option<OffsetDateTime>)>;

#[derive(Debug, Default)]
pub struct InMemoryMembershipCatalog {
    kept: Arc<RwLock<Kept>>,
    people: Arc<RwLock<BTreeMap<String, Person>>>,
    trash: Arc<RwLock<Trash>>,
    leaving: Arc<RwLock<LeavingAccounts>>,
}

impl InMemoryMembershipCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Kept> {
        self.kept.read().expect("membership catalog lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Kept> {
        self.kept.write().expect("membership catalog lock poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let kept = self
            .kept
            .read()
            .expect("membership catalog lock poisoned")
            .clone();
        let people = self
            .people
            .read()
            .expect("membership catalog lock poisoned")
            .clone();
        let trash = self
            .trash
            .read()
            .expect("membership catalog lock poisoned")
            .clone();
        let leaving = self
            .leaving
            .read()
            .expect("membership catalog lock poisoned")
            .clone();
        let into_kept = self.kept.clone();
        let into_people = self.people.clone();
        let into_trash = self.trash.clone();
        let into_leaving = self.leaving.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_kept.write().expect("membership catalog lock poisoned") = kept;
            *into_people
                .write()
                .expect("membership catalog lock poisoned") = people;
            *into_trash
                .write()
                .expect("membership catalog lock poisoned") = trash;
            *into_leaving
                .write()
                .expect("membership catalog lock poisoned") = leaving;
        })
        .map_err(unreachable)
    }
}

fn role_in(members: &ProjectMembers, account: &AccountId) -> Option<Role> {
    members
        .members
        .iter()
        .find(|member| member.account == *account)
        .map(|member| member.role)
}

fn unreachable(failure: Failure) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl MembershipCatalog for InMemoryMembershipCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        members: &ProjectMembers,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut kept = self.write();
        if kept
            .get(members.project.as_str())
            .is_none_or(|known| known.version < members.version)
        {
            kept.insert(members.project.to_string(), members.clone());
        }

        Ok(())
    }

    async fn role_of(
        &self,
        project: &ProjectLink,
        account: &AccountId,
    ) -> Result<Option<Role>, CatalogError> {
        Ok(self
            .read()
            .get(project.as_str())
            .and_then(|members| role_in(members, account)))
    }

    async fn memberships_of(&self, account: &AccountId) -> Result<Vec<Membership>, CatalogError> {
        Ok(self
            .read()
            .values()
            .filter_map(|members| {
                role_in(members, account).map(|role| Membership {
                    project: members.project.clone(),
                    role,
                })
            })
            .collect())
    }

    async fn remember_person(
        &self,
        transaction: &mut Transaction,
        person: &Person,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.people
            .write()
            .expect("membership catalog lock poisoned")
            .insert(person.account.to_string(), person.clone());

        Ok(())
    }

    async fn forget_person(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.people
            .write()
            .expect("membership catalog lock poisoned")
            .remove(account.as_str());

        Ok(())
    }

    async fn people(&self, accounts: &[AccountId]) -> Result<Vec<Person>, CatalogError> {
        let known = self
            .people
            .read()
            .expect("membership catalog lock poisoned");

        Ok(known
            .values()
            .filter(|person| accounts.contains(&person.account))
            .cloned()
            .collect())
    }

    async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut trash = self
            .trash
            .write()
            .expect("membership catalog lock poisoned");
        if trash
            .get(project.as_str())
            .is_none_or(|(known, _)| *known < version)
        {
            trash.insert(project.to_string(), (version, in_the_trash));
        }

        Ok(())
    }

    async fn forget_trash(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.trash
            .write()
            .expect("membership catalog lock poisoned")
            .remove(project.as_str());

        Ok(())
    }

    async fn is_in_the_trash(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
        Ok(self
            .trash
            .read()
            .expect("membership catalog lock poisoned")
            .get(project.as_str())
            .is_some_and(|(_, in_the_trash)| *in_the_trash))
    }

    async fn remember_leaving(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        version: Version,
        until: Option<OffsetDateTime>,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut leaving = self
            .leaving
            .write()
            .expect("membership catalog lock poisoned");
        if leaving
            .get(account.as_str())
            .is_none_or(|(known, _)| *known < version)
        {
            leaving.insert(account.to_string(), (version, until));
        }

        Ok(())
    }

    async fn leaving(&self, accounts: &[AccountId]) -> Result<Vec<Leaving>, CatalogError> {
        let known = self
            .leaving
            .read()
            .expect("membership catalog lock poisoned");

        Ok(known
            .iter()
            .filter_map(|(account, (_, until))| {
                let account = AccountId::from(account.as_str());
                let until = (*until)?;
                accounts
                    .contains(&account)
                    .then_some(Leaving { account, until })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use crate::InMemoryMembershipCatalog;
    use transacting::{InMemoryTransactions, Transactions};

    use crate::suite::Workbench;

    struct InMemory(InMemoryMembershipCatalog, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryMembershipCatalog;

        async fn setup() -> Self {
            Self(
                InMemoryMembershipCatalog::new(),
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
