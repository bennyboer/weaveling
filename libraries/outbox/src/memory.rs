use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use clock::Clock;
use messaging::{Message, Publisher};
use time::OffsetDateTime;
use tokio::sync::Notify;
use transacting::Transaction;

use messaging::Notifications;

use crate::outbox::{CLAIM_FOR, Delivered, Origin, Outbox, OutboxError};

struct Entry {
    entry: i64,
    message: Message,
    claimed_until: Option<OffsetDateTime>,
    published_at: Option<OffsetDateTime>,
}

#[derive(Default)]
struct Entries {
    held: Vec<Entry>,
    last: i64,
}

pub struct InMemoryOutbox {
    entries: Arc<Mutex<Entries>>,
    publisher: Arc<dyn Publisher>,
    clock: Arc<dyn Clock>,
    waiting: Arc<Notify>,
}

struct Waiting(Arc<Notify>);

impl InMemoryOutbox {
    pub fn new(publisher: Arc<dyn Publisher>, clock: Arc<dyn Clock>) -> Self {
        Self {
            entries: Arc::new(Mutex::new(Entries::default())),
            publisher,
            clock,
            waiting: Arc::new(Notify::new()),
        }
    }

    pub fn enqueue(&self, message: Message) {
        put(&self.entries, &self.waiting, message);
    }

    pub fn enqueue_after_commit(&self, transaction: &mut Transaction, message: Message) {
        let (entries, waiting) = (self.entries.clone(), self.waiting.clone());

        transaction.after_commit(async move { put(&entries, &waiting, message) });
    }

    pub fn waiting_to_be_published(&self) -> usize {
        self.held()
            .held
            .iter()
            .filter(|entry| entry.published_at.is_none())
            .count()
    }

    fn held(&self) -> MutexGuard<'_, Entries> {
        self.entries.lock().expect("outbox lock poisoned")
    }

    fn claim(&self, at_most: i64, now: OffsetDateTime) -> Vec<(i64, Message)> {
        self.held()
            .held
            .iter_mut()
            .filter(|entry| entry.published_at.is_none())
            .filter(|entry| entry.claimed_until.is_none_or(|until| until < now))
            .take(at_most.max(0) as usize)
            .map(|entry| {
                entry.claimed_until = Some(now + CLAIM_FOR);

                (entry.entry, entry.message.clone())
            })
            .collect()
    }

    fn release(&self, entries: &[i64]) {
        for held in self.held().held.iter_mut() {
            if entries.contains(&held.entry) {
                held.claimed_until = None;
            }
        }
    }

    fn mark_published(&self, entry: i64, now: OffsetDateTime) {
        if let Some(found) = self.held().held.iter_mut().find(|held| held.entry == entry) {
            found.published_at = Some(now);
        }
    }
}

fn put(entries: &Mutex<Entries>, waiting: &Notify, message: Message) {
    let mut entries = entries.lock().expect("outbox lock poisoned");
    entries.last += 1;
    let entry = entries.last;

    entries.held.push(Entry {
        entry,
        message,
        claimed_until: None,
        published_at: None,
    });
    drop(entries);

    waiting.notify_one();
}

#[async_trait]
impl Outbox for InMemoryOutbox {
    async fn enqueue_within(
        &self,
        transaction: &mut Transaction,
        _origin: Origin<'_>,
        message: &Message,
    ) -> Result<(), OutboxError> {
        self.enqueue_after_commit(transaction, message.clone());

        Ok(())
    }

    async fn deliver(&self, at_most: i64) -> Result<Delivered, OutboxError> {
        let claimed = self.claim(at_most, self.clock.now());

        let mut delivered = Delivered {
            published: 0,
            refused: 0,
        };

        for (at, (entry, message)) in claimed.iter().cloned().enumerate() {
            match self.publisher.publish(message).await {
                Ok(()) => {
                    self.mark_published(entry, self.clock.now());
                    delivered.published += 1;
                }
                Err(undelivered) => {
                    tracing::warn!(entry, error = %undelivered, "an outbox entry could not be published");
                    delivered.refused += 1;
                    let left: Vec<i64> = claimed[at..].iter().map(|(entry, _)| *entry).collect();
                    self.release(&left);
                    break;
                }
            }
        }

        Ok(delivered)
    }

    async fn delete_published(
        &self,
        before: OffsetDateTime,
        at_most: i64,
    ) -> Result<u64, OutboxError> {
        let mut entries = self.held();
        let mut gone = 0;

        entries.held.retain(|entry| {
            let stale = entry
                .published_at
                .is_some_and(|published| published < before);

            if stale && gone < at_most.max(0) as u64 {
                gone += 1;
                return false;
            }

            true
        });

        Ok(gone)
    }

    async fn notifications(&self) -> Result<Box<dyn Notifications>, OutboxError> {
        Ok(Box::new(Waiting(self.waiting.clone())))
    }
}

#[async_trait]
impl Notifications for Waiting {
    async fn wait(&mut self) {
        self.0.notified().await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use clock::FixedClock;
    use messaging::{FakePublisher, Message};
    use time::OffsetDateTime;
    use transacting::{InMemoryTransactions, Transactions};

    use super::InMemoryOutbox;
    use crate::outbox::Outbox;
    use crate::suite;
    use crate::suite::{Then, Workbench};

    struct InMemory(Arc<InMemoryOutbox>);

    #[async_trait]
    impl Workbench for InMemory {
        type Outbox = InMemoryOutbox;

        async fn setup(publisher: Arc<FakePublisher>, clock: Arc<FixedClock>) -> Self {
            Self(Arc::new(InMemoryOutbox::new(publisher, clock)))
        }

        fn outbox(&self) -> &InMemoryOutbox {
            &self.0
        }

        async fn enqueue(&self, messages: &[Message], then: Then) {
            let mut transaction = InMemoryTransactions::new()
                .begin()
                .await
                .expect("a transaction opens");
            for message in messages {
                self.0
                    .enqueue_within(&mut transaction, suite::from_a_scene(), message)
                    .await
                    .expect("enqueueing should succeed");
            }
            match then {
                Then::Commit => transaction.finish().await.expect("the transaction commits"),
                Then::RollBack => drop(transaction),
            }
        }

        async fn published(&self) -> Vec<bool> {
            self.0
                .held()
                .held
                .iter()
                .map(|entry| entry.published_at.is_some())
                .collect()
        }

        async fn all_published_at(&self, when: OffsetDateTime) {
            self.0
                .held()
                .held
                .iter_mut()
                .for_each(|entry| entry.published_at = Some(when));
        }

        async fn cleanup(self) {}
    }

    crate::outbox_conformance_tests!(InMemory);
}
