use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use clock::Clock;
use messaging::{Message, Publisher};
use time::OffsetDateTime;
use tokio::sync::Notify;

use crate::outbox::{CLAIM_FOR, Delivered, Notifications, Outbox, OutboxError};

struct Entry {
    entry: i64,
    message: Message,
    claimed_until: Option<OffsetDateTime>,
    published_at: Option<OffsetDateTime>,
}

pub struct InMemoryOutbox {
    entries: Mutex<Vec<Entry>>,
    publisher: Arc<dyn Publisher>,
    clock: Arc<dyn Clock>,
    waiting: Arc<Notify>,
}

struct Waiting(Arc<Notify>);

impl InMemoryOutbox {
    pub fn new(publisher: Arc<dyn Publisher>, clock: Arc<dyn Clock>) -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
            publisher,
            clock,
            waiting: Arc::new(Notify::new()),
        }
    }

    pub fn enqueue(&self, message: Message) {
        let mut entries = self.held();
        let entry = entries.len() as i64 + 1;

        entries.push(Entry {
            entry,
            message,
            claimed_until: None,
            published_at: None,
        });
        drop(entries);

        self.waiting.notify_one();
    }

    pub fn waiting_to_be_published(&self) -> usize {
        self.held()
            .iter()
            .filter(|entry| entry.published_at.is_none())
            .count()
    }

    fn held(&self) -> MutexGuard<'_, Vec<Entry>> {
        self.entries.lock().expect("outbox lock poisoned")
    }

    fn claim(&self, at_most: i64, now: OffsetDateTime) -> Vec<(i64, Message)> {
        self.held()
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

    fn mark_published(&self, entry: i64, now: OffsetDateTime) {
        if let Some(found) = self.held().iter_mut().find(|held| held.entry == entry) {
            found.published_at = Some(now);
        }
    }
}

#[async_trait]
impl Outbox for InMemoryOutbox {
    async fn deliver(&self, at_most: i64) -> Result<Delivered, OutboxError> {
        let claimed = self.claim(at_most, self.clock.now());

        let mut delivered = Delivered {
            published: 0,
            refused: 0,
        };

        for (entry, message) in claimed {
            match self.publisher.publish(message).await {
                Ok(()) => {
                    self.mark_published(entry, self.clock.now());
                    delivered.published += 1;
                }
                Err(undelivered) => {
                    tracing::warn!(entry, error = %undelivered, "an outbox entry could not be published");
                    delivered.refused += 1;
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

        entries.retain(|entry| {
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
