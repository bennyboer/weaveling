use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use transacting::{Transaction, memory};

use crate::{Belongings, BelongingsError};

type Kept = HashMap<(String, String, String), String>;

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
        self.kept().insert(
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
    ) -> Result<(), BelongingsError> {
        self.undo_on_rollback(transaction)?;
        self.kept()
            .retain(|(held_kind, held_id, _), _| held_kind != kind || held_id != id);

        Ok(())
    }

    async fn owner_of(
        &self,
        kind: &str,
        id: &str,
        owner_kind: &str,
    ) -> Result<Option<String>, BelongingsError> {
        Ok(self
            .kept()
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
