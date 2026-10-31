use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use clock::Clock;
use messaging::{ListenerName, MessageId};
use time::OffsetDateTime;
use transacting::{Transaction, memory};

use crate::{Inbox, InboxError};

type Heard = HashMap<(ListenerName, MessageId), OffsetDateTime>;

pub struct InMemoryInbox {
    heard: Arc<Mutex<Heard>>,
    clock: Arc<dyn Clock>,
}

impl InMemoryInbox {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            heard: Arc::new(Mutex::new(HashMap::new())),
            clock,
        }
    }

    fn held(&self) -> MutexGuard<'_, Heard> {
        self.heard.lock().expect("inbox lock poisoned")
    }

    fn undo_on_rollback(
        &self,
        transaction: &mut Transaction,
        restore: impl FnOnce(&mut Heard) + Send + 'static,
    ) -> Result<(), InboxError> {
        let heard = self.heard.clone();

        Ok(memory::undo_on_rollback(transaction, move || {
            restore(&mut heard.lock().expect("inbox lock poisoned"));
        })?)
    }
}

#[async_trait]
impl Inbox for InMemoryInbox {
    async fn heard_before(
        &self,
        transaction: &mut Transaction,
        listener: &ListenerName,
        message: MessageId,
    ) -> Result<bool, InboxError> {
        let key = (listener.clone(), message);
        let mut heard = self.held();
        if heard.contains_key(&key) {
            return Ok(true);
        }

        let undone = key.clone();
        self.undo_on_rollback(transaction, move |heard| {
            heard.remove(&undone);
        })?;
        heard.insert(key, self.clock.now());

        Ok(false)
    }

    async fn forget_heard_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, InboxError> {
        let mut heard = self.held();
        let mut old: Vec<_> = heard
            .iter()
            .filter(|(_, at)| **at < before)
            .map(|(key, at)| (key.clone(), *at))
            .collect();
        old.sort_by_key(|(_, at)| *at);
        old.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));

        let forgotten = old.clone();
        self.undo_on_rollback(transaction, move |heard| heard.extend(forgotten))?;
        for (key, _) in &old {
            heard.remove(key);
        }

        Ok(old.len() as u64)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use clock::FixedClock;
    use transacting::{InMemoryTransactions, Transactions};

    use super::InMemoryInbox;
    use crate::suite::Workbench;

    struct InMemory {
        inbox: InMemoryInbox,
        transactions: InMemoryTransactions,
    }

    #[async_trait]
    impl Workbench for InMemory {
        type Inbox = InMemoryInbox;

        async fn setup(clock: Arc<FixedClock>) -> Self {
            Self {
                inbox: InMemoryInbox::new(clock),
                transactions: InMemoryTransactions::new(),
            }
        }

        fn inbox(&self) -> &InMemoryInbox {
            &self.inbox
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.transactions
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
