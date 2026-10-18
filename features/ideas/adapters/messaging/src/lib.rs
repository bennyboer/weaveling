mod publishing;
mod sweeping;

pub use publishing::{UnreadableIdeaEvent, event_in, idea_in, message_for};
pub use sweeping::{AT_MOST, DiscardOnProjectDeleted};

use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use eventsourcing::{AggregateId, EventSourcingService, EventStore};
use ideas_contract::EVERY_IDEA;
use ideas_core::{Idea, IdeaCatalog, IdeaEvent, IdeaId, IdeaSummary};
use messaging::{Failure, Handler, Message};

pub struct IdeaCatalogProjector {
    events: EventSourcingService<Idea>,
    catalog: Arc<dyn IdeaCatalog>,
}

impl IdeaCatalogProjector {
    pub fn new(
        store: Arc<dyn EventStore<IdeaEvent>>,
        catalog: Arc<dyn IdeaCatalog>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: EventSourcingService::new(store, clock),
            catalog,
        }
    }

    async fn catalogue(&self, id: &IdeaId) -> Result<(), Failure> {
        let standing = self.events.latest(&AggregateId::from(id)).await?;

        if standing.state.is_discarded() {
            self.catalog.forget(id).await?;
        } else {
            self.catalog
                .remember(&IdeaSummary::of(*id, standing.version, &standing.state))
                .await?;
        }

        Ok(())
    }
}

#[async_trait]
impl Handler for IdeaCatalogProjector {
    const NAME: &'static str = "catalogue-idea";
    const LISTENS_TO: &'static [&'static str] = &[EVERY_IDEA];
    const WHEN_REFUSED: &'static str = "A change to an idea did not reach the list of ideas.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        self.catalogue(&idea_in(message)?).await
    }
}
