use std::sync::Arc;

use eventsourcing::{Cadence, Outbox, RelayTask};

pub struct Relays {
    running: Vec<RelayTask>,
}

impl Relays {
    pub fn started(outboxes: Vec<Arc<dyn Outbox>>, cadence: Cadence) -> Self {
        let running: Vec<RelayTask> = outboxes
            .into_iter()
            .map(|outbox| RelayTask::started(outbox, cadence))
            .collect();

        tracing::info!(relays = running.len(), "started the outbox relays");

        Self { running }
    }

    pub async fn stop(self) {
        for relay in self.running {
            relay.stop().await;
        }
    }
}
