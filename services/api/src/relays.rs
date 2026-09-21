use std::sync::Arc;

use clock::Clock;
use eventsourcing::{Cadence, RelayTask};
use messaging::Publisher;

use crate::databases::Databases;

pub struct Relays {
    running: Vec<RelayTask>,
}

impl Relays {
    pub fn started(
        databases: &Databases,
        publisher: Arc<dyn Publisher>,
        clock: Arc<dyn Clock>,
        cadence: Cadence,
    ) -> Self {
        let asked = [
            projects_wiring::outbox(&databases.projects, publisher.clone(), clock.clone()),
            pieces_wiring::outbox(&databases.pieces, publisher.clone(), clock.clone()),
            boards_wiring::outbox(&databases.boards, publisher.clone(), clock.clone()),
            outline_wiring::outbox(&databases.outline, publisher.clone(), clock.clone()),
            passages_wiring::outbox(&databases.passages, publisher, clock),
        ];

        let running: Vec<RelayTask> = asked
            .into_iter()
            .flatten()
            .map(|outbox| RelayTask::started(Arc::new(outbox), cadence))
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
