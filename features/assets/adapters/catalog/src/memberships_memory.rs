use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard};

use assets_core::{CatalogError, Membership, MembershipCatalog, ProjectLink};
use async_trait::async_trait;
use eventsourcing::AccountId;
use transacting::{Transaction, memory};

type Kept = BTreeSet<(String, String)>;

#[derive(Debug, Default)]
pub struct InMemoryMembershipCatalog {
    kept: Arc<Mutex<Kept>>,
}

impl InMemoryMembershipCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    fn kept(&self) -> MutexGuard<'_, Kept> {
        self.kept
            .lock()
            .expect("the membership catalog is not poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), CatalogError> {
        let before = self.kept().clone();
        let into = self.kept.clone();

        memory::undo_on_rollback(transaction, move || {
            *into.lock().expect("the membership catalog is not poisoned") = before;
        })
        .map_err(|failure| CatalogError::Backend(Box::new(failure)))
    }

    fn projects_of(&self, account: &AccountId) -> BTreeSet<String> {
        let account = account.to_string();

        self.kept()
            .iter()
            .filter(|(_, member)| *member == account)
            .map(|(project, _)| project.clone())
            .collect()
    }
}

fn held(membership: &Membership) -> (String, String) {
    (
        membership.project.to_string(),
        membership.account.to_string(),
    )
}

#[async_trait]
impl MembershipCatalog for InMemoryMembershipCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        membership: &Membership,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.kept().insert(held(membership));

        Ok(())
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        membership: &Membership,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.kept().remove(&held(membership));

        Ok(())
    }

    async fn forget_project(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let project = project.to_string();
        self.kept().retain(|(held, _)| *held != project);

        Ok(())
    }

    async fn share_a_project(
        &self,
        _transaction: &mut Transaction,
        one: &AccountId,
        other: &AccountId,
    ) -> Result<bool, CatalogError> {
        let others = self.projects_of(other);

        Ok(self
            .projects_of(one)
            .iter()
            .any(|project| others.contains(project)))
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;
    use crate::memberships_suite::Workbench;

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

    crate::membership_conformance_tests!(InMemory);
}
