mod publishing;
mod sweeping;

pub use publishing::{
    IdeaEventPublisher, UnreadableIdeaEvent, event_in, every_event, idea_in, message_for,
};
pub use sweeping::{AT_MOST, DiscardOnProjectDeleted, when_more_to_sweep, when_project_deleted};

use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use eventsourcing::{AggregateId, EventSourcingService, EventStore, ServiceError};
use ideas_core::{CatalogError, Idea, IdeaCatalog, IdeaError, IdeaEvent, IdeaId, IdeaSummary};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use thiserror::Error;

const NAME: &str = "catalogue-idea";

pub struct IdeaCatalogProjector {
    events: EventSourcingService<Idea>,
    catalog: Arc<dyn IdeaCatalog>,
}

#[derive(Debug, Error)]
enum NotCatalogued {
    #[error(transparent)]
    Unreadable(#[from] UnreadableIdeaEvent),
    #[error(transparent)]
    Events(#[from] ServiceError<IdeaError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
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

    async fn catalogue(&self, id: &IdeaId) -> Result<(), NotCatalogued> {
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

    async fn handle(&self, message: &Message) -> Result<(), NotCatalogued> {
        self.catalogue(&idea_in(message)?).await
    }
}

#[async_trait]
impl Listener for IdeaCatalogProjector {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the catalog listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![every_event()]
    }

    fn delivery(&self) -> Delivery {
        Delivery::Kept
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        self.handle(message)
            .await
            .map_err(|why| NotHandled::because(self.named(), message.routing.clone(), why))
    }
}
