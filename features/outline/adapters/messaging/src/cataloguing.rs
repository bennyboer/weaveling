use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use eventsourcing::{AggregateId, EventSourcingService, EventStore, ServiceError};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use outline_core::{
    CatalogError, Outline, OutlineCatalog, OutlineError, OutlineEvent, OutlineId, OutlineSummary,
};
use thiserror::Error;

use crate::publishing::{UnreadableOutlineEvent, outline_in, when_started};

const NAME: &str = "catalogue-outline";

pub struct OutlineCatalogProjector {
    events: EventSourcingService<Outline>,
    catalog: Arc<dyn OutlineCatalog>,
}

#[derive(Debug, Error)]
enum NotCatalogued {
    #[error(transparent)]
    Unreadable(#[from] UnreadableOutlineEvent),
    #[error(transparent)]
    Events(#[from] ServiceError<OutlineError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
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

    async fn catalogue(&self, id: &OutlineId) -> Result<(), NotCatalogued> {
        let standing = self.events.latest(&AggregateId::from(id)).await?;

        self.catalog
            .remember(&OutlineSummary::of(*id, &standing.state))
            .await?;

        Ok(())
    }

    async fn work_through(&self, message: &Message) -> Result<(), NotCatalogued> {
        self.catalogue(&outline_in(message)?).await
    }
}

#[async_trait]
impl Listener for OutlineCatalogProjector {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the catalog listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![when_started()]
    }

    fn delivery(&self) -> Delivery {
        Delivery::Kept
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        self.work_through(message)
            .await
            .map_err(|why| NotHandled::because(self.named(), message.routing.clone(), why))
    }
}
