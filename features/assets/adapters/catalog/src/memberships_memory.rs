use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};

use assets_core::{CatalogError, Membership, MembershipCatalog, ProjectLink};
use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use time::OffsetDateTime;
use transacting::{Transaction, memory};

type Key = (String, String);

#[derive(Debug, Clone, Default)]
struct Kept {
    memberships: HashMap<Key, (u64, Option<OffsetDateTime>)>,
    disbanded: HashMap<String, (u64, OffsetDateTime)>,
}

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

    fn told(
        &self,
        transaction: &mut Transaction,
        membership: &Membership,
        version: Version,
        gone_at: Option<OffsetDateTime>,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut kept = self.kept();
        let held = kept
            .memberships
            .entry(key(membership))
            .or_insert((0, gone_at));
        if held.0 < version.count() {
            *held = (version.count(), gone_at);
        }

        Ok(())
    }

    fn projects_of(kept: &Kept, account: &AccountId) -> BTreeSet<String> {
        let account = account.to_string();

        kept.memberships
            .iter()
            .filter(|((project, member), (_, gone_at))| {
                *member == account && gone_at.is_none() && !kept.disbanded.contains_key(project)
            })
            .map(|((project, _), _)| project.clone())
            .collect()
    }
}

fn key(membership: &Membership) -> Key {
    (
        membership.project.to_string(),
        membership.account.to_string(),
    )
}

fn oldest<K: Clone + Ord>(
    laid: impl Iterator<Item = (OffsetDateTime, K)>,
    before: OffsetDateTime,
    at_most: u64,
) -> Vec<K> {
    let mut old: Vec<(OffsetDateTime, K)> = laid.filter(|(at, _)| *at < before).collect();
    old.sort();
    old.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));

    old.into_iter().map(|(_, laid)| laid).collect()
}

#[async_trait]
impl MembershipCatalog for InMemoryMembershipCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        membership: &Membership,
        version: Version,
    ) -> Result<(), CatalogError> {
        self.told(transaction, membership, version, None)
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        membership: &Membership,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.told(transaction, membership, version, Some(at))
    }

    async fn forget_project(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.undo_on_rollback(transaction)?;
        self.kept()
            .disbanded
            .entry(project.to_string())
            .or_insert((version.count(), at));

        Ok(())
    }

    async fn share_a_project(
        &self,
        _transaction: &mut Transaction,
        one: &AccountId,
        other: &AccountId,
    ) -> Result<bool, CatalogError> {
        let kept = self.kept();
        let others = Self::projects_of(&kept, other);

        Ok(Self::projects_of(&kept, one)
            .iter()
            .any(|project| others.contains(project)))
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.undo_on_rollback(transaction)?;
        let mut kept = self.kept();

        let disbanded = oldest(
            kept.disbanded
                .iter()
                .map(|(project, (_, laid_at))| (*laid_at, project.clone())),
            before,
            at_most,
        );
        for project in &disbanded {
            kept.disbanded.remove(project);
            kept.memberships
                .retain(|(held_in, _), _| held_in != project);
        }
        let gone = oldest(
            kept.memberships
                .iter()
                .filter_map(|(key, (_, gone_at))| gone_at.map(|at| (at, key.clone()))),
            before,
            at_most,
        );
        for key in &gone {
            kept.memberships.remove(key);
        }

        Ok((disbanded.len() + gone.len()) as u64)
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
