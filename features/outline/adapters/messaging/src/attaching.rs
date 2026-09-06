use std::sync::Arc;

use clock::Clock;
use eventsourcing::{AggregateId, EventSourcingService, EventStore, ServiceError};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use outline_core::{
    CatalogError, Outline, OutlineCatalog, OutlineError, OutlineEvent, OutlineId, PieceLink,
};
use thiserror::Error;

use crate::publishing::{
    UnreadableOutlineEvent, outline_in, when_attached, when_detached, when_section_removed,
};

const NAME: &str = "index-attached-pieces";

pub struct AttachedPiecesProjector {
    events: EventSourcingService<Outline>,
    catalog: Arc<dyn OutlineCatalog>,
}

#[derive(Debug, Error)]
enum NotIndexed {
    #[error(transparent)]
    Unreadable(#[from] UnreadableOutlineEvent),
    #[error(transparent)]
    Events(#[from] ServiceError<OutlineError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

impl AttachedPiecesProjector {
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

    async fn index(&self, outline: &OutlineId) -> Result<(), NotIndexed> {
        let standing = self.events.latest(&AggregateId::from(outline)).await?;
        let holding: Vec<PieceLink> = standing.state.reading_order();

        self.catalog.holds(*outline, &holding).await?;

        Ok(())
    }

    async fn work_through(&self, message: &Message) -> Result<(), NotIndexed> {
        self.index(&outline_in(message)?).await
    }
}

#[async_trait::async_trait]
impl Listener for AttachedPiecesProjector {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the attachment index listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![when_attached(), when_detached(), when_section_removed()]
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
