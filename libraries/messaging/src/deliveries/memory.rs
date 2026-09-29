use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use time::OffsetDateTime;

use crate::delivering::{CLAIM_FOR, DeadLetter, Deliveries, DeliveryError, Queued};
use crate::listening::ListenerName;
use crate::message::Message;

struct QueuedDelivery {
    id: i64,
    listener: ListenerName,
    message: Message,
    attempts: i32,
    due_at: OffsetDateTime,
    claimed_until: Option<OffsetDateTime>,
}

#[derive(Default)]
struct Queues {
    deliveries: Vec<QueuedDelivery>,
    dead_letters: Vec<DeadLetter>,
    last_id: i64,
}

#[derive(Default)]
pub struct InMemoryDeliveries {
    queues: Mutex<Queues>,
}

impl InMemoryDeliveries {
    pub fn new() -> Self {
        Self::default()
    }

    fn queues(&self) -> MutexGuard<'_, Queues> {
        self.queues.lock().expect("deliveries lock poisoned")
    }
}

#[async_trait]
impl Deliveries for InMemoryDeliveries {
    async fn enqueue(
        &self,
        listener: &ListenerName,
        message: &Message,
    ) -> Result<(), DeliveryError> {
        let mut queues = self.queues();
        queues.last_id += 1;
        let id = queues.last_id;

        queues.deliveries.push(QueuedDelivery {
            id,
            listener: listener.clone(),
            message: message.clone(),
            attempts: 0,
            due_at: message.occurred_at,
            claimed_until: None,
        });

        Ok(())
    }

    async fn claim_due(
        &self,
        now: OffsetDateTime,
        at_most: i64,
    ) -> Result<Vec<Queued>, DeliveryError> {
        Ok(self
            .queues()
            .deliveries
            .iter_mut()
            .filter(|queued| queued.due_at <= now)
            .filter(|queued| queued.claimed_until.is_none_or(|until| until < now))
            .take(at_most.max(0) as usize)
            .map(|queued| {
                queued.claimed_until = Some(now + CLAIM_FOR);

                Queued {
                    id: queued.id,
                    listener: queued.listener.clone(),
                    message: queued.message.clone(),
                    attempts: queued.attempts,
                }
            })
            .collect())
    }

    async fn mark_as_handled(&self, delivery: i64) -> Result<(), DeliveryError> {
        self.queues()
            .deliveries
            .retain(|queued| queued.id != delivery);

        Ok(())
    }

    async fn mark_as_refused(
        &self,
        delivery: i64,
        _why: &str,
        again_at: OffsetDateTime,
    ) -> Result<(), DeliveryError> {
        if let Some(found) = self
            .queues()
            .deliveries
            .iter_mut()
            .find(|queued| queued.id == delivery)
        {
            found.attempts += 1;
            found.due_at = again_at;
            found.claimed_until = None;
        }

        Ok(())
    }

    async fn give_up(&self, delivery: i64, why: &str) -> Result<(), DeliveryError> {
        let mut queues = self.queues();

        let Some(at) = queues
            .deliveries
            .iter()
            .position(|queued| queued.id == delivery)
        else {
            return Ok(());
        };
        let gone = queues.deliveries.remove(at);

        queues.dead_letters.push(DeadLetter {
            listener: gone.listener,
            message: gone.message,
            attempts: gone.attempts + 1,
            why: why.to_owned(),
        });

        Ok(())
    }

    async fn dead_letters(&self) -> Result<Vec<DeadLetter>, DeliveryError> {
        Ok(self.queues().dead_letters.clone())
    }

    async fn waiting(&self) -> Result<usize, DeliveryError> {
        Ok(self.queues().deliveries.len())
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
