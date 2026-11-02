use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use time::OffsetDateTime;

use crate::delivering::{CLAIM_FOR, Claim, DeadLetter, Deliveries, DeliveryError, Queued};
use crate::listening::{ListenerName, Notifications};
use crate::message::Message;
use tokio::sync::Notify;

struct QueuedDelivery {
    id: i64,
    listener: ListenerName,
    message: Message,
    attempts: i32,
    due_at: OffsetDateTime,
    claimed_until: Option<OffsetDateTime>,
    claim: Option<Claim>,
}

#[derive(Default)]
struct Queues {
    deliveries: Vec<QueuedDelivery>,
    dead_letters: Vec<DeadLetter>,
    last_id: i64,
    last_dead_letter: i64,
}

#[derive(Default)]
pub struct InMemoryDeliveries {
    queues: Mutex<Queues>,
    waiting: Arc<Notify>,
}

struct Waiting(Arc<Notify>);

#[async_trait]
impl Notifications for Waiting {
    async fn wait(&mut self) {
        self.0.notified().await;
    }
}

impl InMemoryDeliveries {
    pub fn new() -> Self {
        Self::default()
    }

    fn queues(&self) -> MutexGuard<'_, Queues> {
        self.queues.lock().expect("deliveries lock poisoned")
    }
}

impl Queues {
    fn queue(&mut self, listener: ListenerName, message: Message, due_at: OffsetDateTime) {
        if self
            .deliveries
            .iter()
            .any(|queued| queued.listener == listener && queued.message.id == message.id)
        {
            return;
        }
        self.last_id += 1;

        self.deliveries.push(QueuedDelivery {
            id: self.last_id,
            listener,
            message,
            attempts: 0,
            due_at,
            claimed_until: None,
            claim: None,
        });
    }

    fn held_by(&mut self, queued: &Queued) -> Option<usize> {
        self.deliveries
            .iter()
            .position(|held| held.id == queued.id && held.claim == Some(queued.claim))
    }
}

#[async_trait]
impl Deliveries for InMemoryDeliveries {
    async fn enqueue(
        &self,
        listener: &ListenerName,
        message: &Message,
    ) -> Result<(), DeliveryError> {
        self.queues()
            .queue(listener.clone(), message.clone(), message.occurred_at);

        self.waiting.notify_one();

        Ok(())
    }

    async fn claim_due(
        &self,
        now: OffsetDateTime,
        at_most: i64,
    ) -> Result<Vec<Queued>, DeliveryError> {
        let claim = Claim::drawn();

        Ok(self
            .queues()
            .deliveries
            .iter_mut()
            .filter(|queued| queued.due_at <= now)
            .filter(|queued| queued.claimed_until.is_none_or(|until| until < now))
            .take(at_most.max(0) as usize)
            .map(|queued| {
                queued.claimed_until = Some(now + CLAIM_FOR);
                queued.claim = Some(claim);

                Queued {
                    id: queued.id,
                    claim,
                    listener: queued.listener.clone(),
                    message: queued.message.clone(),
                    attempts: queued.attempts,
                }
            })
            .collect())
    }

    async fn mark_as_handled(&self, queued: &Queued) -> Result<(), DeliveryError> {
        let mut queues = self.queues();
        if let Some(nth) = queues.held_by(queued) {
            queues.deliveries.remove(nth);
        }

        Ok(())
    }

    async fn mark_as_refused(
        &self,
        queued: &Queued,
        _why: &str,
        again_at: OffsetDateTime,
    ) -> Result<(), DeliveryError> {
        let mut queues = self.queues();
        if let Some(nth) = queues.held_by(queued) {
            let found = &mut queues.deliveries[nth];
            found.attempts += 1;
            found.due_at = again_at;
            found.claimed_until = None;
            found.claim = None;
        }

        Ok(())
    }

    async fn give_up(
        &self,
        queued: &Queued,
        why: &str,
        at: OffsetDateTime,
    ) -> Result<(), DeliveryError> {
        let mut queues = self.queues();

        let Some(nth) = queues.held_by(queued) else {
            return Ok(());
        };
        let gone = queues.deliveries.remove(nth);
        queues.last_dead_letter += 1;
        let id = queues.last_dead_letter;

        queues.dead_letters.push(DeadLetter {
            id,
            listener: gone.listener,
            message: gone.message,
            attempts: gone.attempts + 1,
            why: why.to_owned(),
            given_up_at: at,
            acknowledged_at: None,
        });

        Ok(())
    }

    async fn dead_letters(&self) -> Result<Vec<DeadLetter>, DeliveryError> {
        Ok(self.queues().dead_letters.clone())
    }

    async fn retry(&self, dead_letter: i64, at: OffsetDateTime) -> Result<(), DeliveryError> {
        let mut queues = self.queues();

        let Some(nth) = queues
            .dead_letters
            .iter()
            .position(|dead| dead.id == dead_letter)
        else {
            return Ok(());
        };
        let revived = queues.dead_letters.remove(nth);
        queues.queue(revived.listener, revived.message, at);
        drop(queues);

        self.waiting.notify_one();

        Ok(())
    }

    async fn acknowledge(&self, dead_letter: i64, at: OffsetDateTime) -> Result<(), DeliveryError> {
        if let Some(found) = self
            .queues()
            .dead_letters
            .iter_mut()
            .find(|dead| dead.id == dead_letter)
        {
            found.acknowledged_at.get_or_insert(at);
        }

        Ok(())
    }

    async fn waiting(&self) -> Result<usize, DeliveryError> {
        Ok(self.queues().deliveries.len())
    }

    async fn notifications(&self) -> Result<Box<dyn Notifications>, DeliveryError> {
        Ok(Box::new(Waiting(self.waiting.clone())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::deliveries::suite::Workbench;

    struct InMemory(InMemoryDeliveries);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryDeliveries;

        async fn setup() -> Self {
            Self(InMemoryDeliveries::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
