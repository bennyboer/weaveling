use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use eventsourcing::AccountId;
use members_core::{
    CatalogError, Membership, MembershipCatalog, Person, ProjectLink, ProjectMembers, Role,
};
use transacting::{Failure, Transaction};

type Kept = BTreeMap<String, ProjectMembers>;

#[derive(Debug, Default)]
pub struct InMemoryMembershipCatalog {
    kept: Arc<RwLock<Kept>>,
    people: Arc<RwLock<BTreeMap<String, Person>>>,
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
        let into_kept = self.kept.clone();
        let into_people = self.people.clone();

        transacting::memory::undo_on_rollback(transaction, move || {
            *into_kept.write().expect("membership catalog lock poisoned") = kept;
            *into_people
                .write()
                .expect("membership catalog lock poisoned") = people;
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
