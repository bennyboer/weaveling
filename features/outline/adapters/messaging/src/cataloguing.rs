use outline_contract::{DISCARDED, STARTED};
use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use eventsourcing::{AggregateId, EventSourcingService, EventStore};
use messaging::{Failure, Handler, Message};
use outline_core::{Outline, OutlineCatalog, OutlineEvent, OutlineId, OutlineSummary};

use crate::publishing::outline_in;

pub struct OutlineCatalogProjector {
    events: EventSourcingService<Outline>,
    catalog: Arc<dyn OutlineCatalog>,
}

impl OutlineCatalogProjector {
    pub fn new(
        store: Arc<dyn EventStore<OutlineEvent>>,
        catalog: Arc<dyn OutlineCatalog>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: EventSourcingService::new(store, clock),
            catalog,
        }
    }

    async fn catalogue(&self, id: &OutlineId) -> Result<(), Failure> {
        let standing = self.events.latest(&AggregateId::from(id)).await?;

        if standing.state.is_discarded() {
            self.catalog.forget(id).await?;
        } else {
            self.catalog
                .remember(&OutlineSummary::of(*id, &standing.state))
                .await?;
        }

        Ok(())
    }
}

#[async_trait]
impl Handler for OutlineCatalogProjector {
    const NAME: &'static str = "catalogue-outline";
    const LISTENS_TO: &'static [&'static str] = &[STARTED, DISCARDED];
    const WHEN_REFUSED: &'static str = "An outline did not reach the list of outlines.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        self.catalogue(&outline_in(message)?).await
    }
}
