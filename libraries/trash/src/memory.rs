use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use time::OffsetDateTime;
use transacting::{Transaction, memory};

use crate::{Trash, TrashError, Trashed};

type Kept = HashMap<(String, String), OffsetDateTime>;

#[derive(Default)]
pub struct InMemoryTrash {
    kept: Arc<Mutex<Kept>>,
}

impl InMemoryTrash {
    pub fn new() -> Self {
        Self::default()
    }

    fn held(&self) -> MutexGuard<'_, Kept> {
        self.kept.lock().expect("trash lock poisoned")
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), TrashError> {
        let (before, kept) = (self.held().clone(), self.kept.clone());

        Ok(memory::undo_on_rollback(transaction, move || {
            *kept.lock().expect("trash lock poisoned") = before;
        })?)
    }
}

#[async_trait]
impl Trash for InMemoryTrash {
    async fn put_into(
        &self,
        transaction: &mut Transaction,
        kind: &str,
        id: &str,
        until: OffsetDateTime,
    ) -> Result<(), TrashError> {
        self.undo_on_rollback(transaction)?;
        self.held().insert((kind.to_owned(), id.to_owned()), until);

        Ok(())
    }

    async fn take_out(
        &self,
        transaction: &mut Transaction,
        kind: &str,
        id: &str,
    ) -> Result<bool, TrashError> {
        self.undo_on_rollback(transaction)?;

        Ok(self
            .held()
            .remove(&(kind.to_owned(), id.to_owned()))
            .is_some())
    }

    async fn due(&self, at: OffsetDateTime, at_most: u64) -> Result<Vec<Trashed>, TrashError> {
        let mut due: Vec<(OffsetDateTime, String, String)> = self
            .held()
            .iter()
            .filter(|(_, kept_until)| **kept_until <= at)
            .map(|((kind, id), kept_until)| (*kept_until, kind.clone(), id.clone()))
            .collect();
        due.sort();
        due.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));

        Ok(due
            .into_iter()
            .map(|(_, kind, id)| Trashed { kind, id })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use transacting::{InMemoryTransactions, Transactions};

    use super::InMemoryTrash;
    use crate::suite::Workbench;

    struct InMemory {
        trash: InMemoryTrash,
        transactions: InMemoryTransactions,
    }

    #[async_trait]
    impl Workbench for InMemory {
        type Trash = InMemoryTrash;

        async fn setup() -> Self {
            Self {
                trash: InMemoryTrash::new(),
                transactions: InMemoryTransactions::new(),
            }
        }

        fn trash(&self) -> &InMemoryTrash {
            &self.trash
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.transactions
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
