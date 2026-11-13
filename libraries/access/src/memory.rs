use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use time::OffsetDateTime;
use transacting::{Transaction, memory};

use crate::{AccessCatalog, AccessError, Role};

type Key = (String, String);

#[derive(Debug, Clone, Default)]
struct Kept {
    roles: HashMap<Key, (u64, Option<Role>, Option<OffsetDateTime>)>,
    disbanded: HashMap<String, (u64, OffsetDateTime)>,
    trash: HashMap<String, (u64, bool)>,
    operators: HashMap<String, (u64, bool, Option<OffsetDateTime>)>,
}

impl Kept {
    fn projects_of(&self, account: &AccountId) -> BTreeSet<String> {
        let account = account.to_string();

        self.roles
            .iter()
            .filter(|((project, member), (_, role, _))| {
                *member == account
                    && role.is_some()
                    && !self.disbanded.contains_key(project)
                    && !self.trash.get(project).is_some_and(|(_, trashed)| *trashed)
            })
            .map(|((project, _), _)| project.clone())
            .collect()
    }
}

#[derive(Debug, Default)]
pub struct InMemoryAccess {
    kept: Arc<Mutex<Kept>>,
}

impl InMemoryAccess {
    pub fn new() -> Self {
        Self::default()
    }

    fn kept(&self) -> MutexGuard<'_, Kept> {
        self.kept.lock().expect("who may do what is not poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), AccessError> {
        let before = self.kept().clone();
        let into = self.kept.clone();

        memory::undo_on_rollback(transaction, move || {
            *into.lock().expect("who may do what is not poisoned") = before;
        })
        .map_err(|failure| AccessError::Backend(Box::new(failure)))
    }
}

fn newer<T>(held: &mut (u64, T), version: Version, told: T) {
    if held.0 < version.count() {
        *held = (version.count(), told);
    }
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
impl AccessCatalog for InMemoryAccess {
    async fn remember_role(
        &self,
        transaction: &mut Transaction,
        project: &str,
        account: &AccountId,
        version: Version,
        role: Option<Role>,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.undo_on_rollback(transaction)?;
        let gone_at = role.is_none().then_some(at);
        let mut kept = self.kept();
        let held = kept
            .roles
            .entry((project.to_owned(), account.to_string()))
            .or_insert((0, role, gone_at));
        if held.0 < version.count() {
            *held = (version.count(), role, gone_at);
        }

        Ok(())
    }

    async fn remember_disbanded(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.undo_on_rollback(transaction)?;
        self.kept()
            .disbanded
            .entry(project.to_owned())
            .or_insert((version.count(), at));

        Ok(())
    }

    async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), AccessError> {
        self.undo_on_rollback(transaction)?;
        let mut kept = self.kept();
        let held = kept
            .trash
            .entry(project.to_owned())
            .or_insert((0, in_the_trash));
        newer(held, version, in_the_trash);

        Ok(())
    }

    async fn remember_operator(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        version: Version,
        operator: bool,
        at: OffsetDateTime,
    ) -> Result<(), AccessError> {
        self.undo_on_rollback(transaction)?;
        let gone_at = (!operator).then_some(at);
        let mut kept = self.kept();
        let held = kept
            .operators
            .entry(account.to_string())
            .or_insert((0, operator, gone_at));
        if held.0 < version.count() {
            *held = (version.count(), operator, gone_at);
        }

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, AccessError> {
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
            kept.trash.remove(project);
            kept.roles.retain(|(held_in, _), _| held_in != project);
        }
        let gone = oldest(
            kept.roles
                .iter()
                .filter_map(|(key, (_, _, gone_at))| gone_at.map(|at| (at, key.clone()))),
            before,
            at_most,
        );
        for key in &gone {
            kept.roles.remove(key);
        }
        let stopped = oldest(
            kept.operators
                .iter()
                .filter_map(|(account, (_, _, gone_at))| gone_at.map(|at| (at, account.clone()))),
            before,
            at_most,
        );
        for account in &stopped {
            kept.operators.remove(account);
        }

        Ok((disbanded.len() + gone.len() + stopped.len()) as u64)
    }

    async fn role_of(
        &self,
        project: &str,
        account: &AccountId,
    ) -> Result<Option<Role>, AccessError> {
        let kept = self.kept();
        if kept.disbanded.contains_key(project) {
            return Ok(None);
        }

        Ok(kept
            .roles
            .get(&(project.to_owned(), account.to_string()))
            .and_then(|(_, role, _)| *role))
    }

    async fn is_in_the_trash(&self, project: &str) -> Result<bool, AccessError> {
        Ok(self
            .kept()
            .trash
            .get(project)
            .is_some_and(|(_, trashed)| *trashed))
    }

    async fn projects_of(&self, account: &AccountId) -> Result<Vec<String>, AccessError> {
        Ok(self.kept().projects_of(account).into_iter().collect())
    }

    async fn share_a_project(
        &self,
        one: &AccountId,
        other: &AccountId,
    ) -> Result<bool, AccessError> {
        let kept = self.kept();
        let others = kept.projects_of(other);

        Ok(kept
            .projects_of(one)
            .iter()
            .any(|project| others.contains(project)))
    }

    async fn operates(&self, account: &AccountId) -> Result<bool, AccessError> {
        Ok(self
            .kept()
            .operators
            .get(&account.to_string())
            .is_some_and(|(_, operator, _)| *operator))
    }

    async fn owners_of(&self, project: &str) -> Result<Vec<AccountId>, AccessError> {
        let kept = self.kept();
        if kept.disbanded.contains_key(project) {
            return Ok(Vec::new());
        }
        let mut owners: Vec<String> = kept
            .roles
            .iter()
            .filter(|((held_in, _), (_, role, _))| held_in == project && *role == Some(Role::Owner))
            .map(|((_, account), _)| account.clone())
            .collect();
        owners.sort();

        Ok(owners.into_iter().map(AccountId::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;
    use crate::suite::Workbench;

    struct InMemory(InMemoryAccess, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryAccess;

        async fn setup() -> Self {
            Self(InMemoryAccess::new(), InMemoryTransactions::new())
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
