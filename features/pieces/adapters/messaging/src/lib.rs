mod publishing;

pub use publishing::{
    Publishing, UnreadablePieceEvent, event_in, every_piece, message_for, piece_in,
};

use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use eventsourcing::{AggregateId, EventSourcingService, EventStore, ServiceError};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use pieces_core::{
    CatalogError, Piece, PieceCatalog, PieceError, PieceEvent, PieceId, PieceSummary,
};
use thiserror::Error;

const NAME: &str = "catalogue-piece";

pub struct PieceCatalogProjector {
    events: EventSourcingService<Piece>,
    catalog: Arc<dyn PieceCatalog>,
}

#[derive(Debug, Error)]
enum NotCatalogued {
    #[error(transparent)]
    Unreadable(#[from] UnreadablePieceEvent),
    #[error(transparent)]
    Events(#[from] ServiceError<PieceError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

impl PieceCatalogProjector {
    pub fn new(
        store: Arc<dyn EventStore<PieceEvent>>,
        catalog: Arc<dyn PieceCatalog>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: EventSourcingService::new(store, clock),
            catalog,
        }
    }

    async fn catalogue(&self, id: &PieceId) -> Result<(), NotCatalogued> {
        let standing = self.events.latest(&AggregateId::from(id)).await?;

        if standing.state.is_discarded() {
            self.catalog.forget(id).await?;
        } else {
            self.catalog
                .remember(&PieceSummary::of(*id, standing.version, &standing.state))
                .await?;
        }

        Ok(())
    }

    async fn handle(&self, message: &Message) -> Result<(), NotCatalogued> {
        self.catalogue(&piece_in(message)?).await
    }
}

#[async_trait]
impl Listener for PieceCatalogProjector {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the catalog listener is named at compile time")
    }

    fn listens_to(&self) -> Subscription {
        every_piece()
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
