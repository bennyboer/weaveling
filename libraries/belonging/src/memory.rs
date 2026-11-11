use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use time::OffsetDateTime;
use transacting::{Transaction, memory};

use crate::{Belongings, BelongingsError};

#[derive(Debug, Clone, Default)]
struct Kept {
    owners: HashMap<(String, String, String), String>,
    tombstones: HashMap<(String, String), OffsetDateTime>,
}

#[derive(Debug, Default)]
pub struct InMemoryBelongings {
    kept: Arc<Mutex<Kept>>,
}

impl InMemoryBelongings {
    pub fn new() -> Self {
        Self::default()
    }

    fn kept(&self) -> MutexGuard<'_, Kept> {
        self.kept.lock().expect("belongings lock poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), BelongingsError> {
        let before = self.kept().clone();
        let into = self.kept.clone();

        memory::undo_on_rollback(transaction, move || {
            *into.lock().expect("belongings lock poisoned") = before;
        })
        .map_err(|failure| BelongingsError::Backend(Box::new(failure)))
    }
}

#[async_trait]
impl Belongings for InMemoryBelongings {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        kind: &str,
        id: &str,
        owner_kind: &str,
        owner: &str,
    ) -> Result<(), BelongingsError> {
        self.undo_on_rollback(transaction)?;
        let mut kept = self.kept();
        if kept
            .tombstones
            .contains_key(&(kind.to_owned(), id.to_owned()))
        {
            return Ok(());
        }
        kept.owners.insert(
            (kind.to_owned(), id.to_owned(), owner_kind.to_owned()),
            owner.to_owned(),
        );

        Ok(())
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        kind: &str,
        id: &str,
        at: OffsetDateTime,
    ) -> Result<(), BelongingsError> {
        self.undo_on_rollback(transaction)?;
        let mut kept = self.kept();
        kept.owners
            .retain(|(held_kind, held_id, _), _| held_kind != kind || held_id != id);
        kept.tombstones
            .entry((kind.to_owned(), id.to_owned()))
            .or_insert(at);

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, BelongingsError> {
        self.undo_on_rollback(transaction)?;
        let mut kept = self.kept();
        let mut old: Vec<(OffsetDateTime, (String, String))> = kept
            .tombstones
            .iter()
            .filter(|(_, laid)| **laid < before)
            .map(|(thing, laid)| (*laid, thing.clone()))
            .collect();
        old.sort();
        old.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));
        for (_, thing) in &old {
            kept.tombstones.remove(thing);
        }

        Ok(u64::try_from(old.len()).unwrap_or(u64::MAX))
    }

    async fn owner_of(
        &self,
        kind: &str,
        id: &str,
        owner_kind: &str,
    ) -> Result<Option<String>, BelongingsError> {
        Ok(self
            .kept()
            .owners
            .get(&(kind.to_owned(), id.to_owned(), owner_kind.to_owned()))
            .cloned())
    }
}

#[cfg(test)]
mod tests {
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;

    use crate::suite::Workbench;

    struct InMemory(InMemoryBelongings, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryBelongings;

        async fn setup() -> Self {
            Self(InMemoryBelongings::new(), InMemoryTransactions::new())
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
