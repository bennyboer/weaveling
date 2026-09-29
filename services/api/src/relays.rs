use std::sync::Arc;

use eventsourcing::{Cadence, Outbox, RelayTask};
use messaging::DeliveryConsumer;
use tokio::sync::watch;
use tokio::task::JoinHandle;

pub struct Relays {
    running: Vec<RelayTask>,
    stopping: watch::Sender<bool>,
    consuming: JoinHandle<()>,
}

impl Relays {
    pub fn started(
        outboxes: Vec<Arc<dyn Outbox>>,
        consuming: DeliveryConsumer,
        cadence: Cadence,
    ) -> Self {
        let running: Vec<RelayTask> = outboxes
            .into_iter()
            .map(|outbox| RelayTask::started(outbox, cadence))
            .collect();
        let (stopping, stopped) = watch::channel(false);

        tracing::info!(relays = running.len(), "started the outbox relays");

        Self {
            running,
            stopping,
            consuming: tokio::spawn(draining(consuming, cadence, stopped)),
        }
    }

    pub async fn stop(self) {
        let _ = self.stopping.send(true);
        let _ = self.consuming.await;

        for relay in self.running {
            relay.stop().await;
        }
    }
}

async fn draining(
    consuming: DeliveryConsumer,
    cadence: Cadence,
    mut stopped: watch::Receiver<bool>,
) {
    loop {
        consuming.drain(cadence.deliver_at_most).await;

        tokio::select! {
            _ = stopped.changed() => break,
            _ = tokio::time::sleep(cadence.deliver_every) => {}
        }
    }
}
