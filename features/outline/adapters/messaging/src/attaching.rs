use std::sync::Arc;

use clock::Clock;
use eventsourcing::{AggregateId, EventSourcingService, EventStore, ServiceError};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use outline_core::{
    CatalogError, Outline, OutlineCatalog, OutlineError, OutlineEvent, OutlineId, OutlineService,
};
use thiserror::Error;

use crate::letting_go::{NotLetGo, let_go_of_what_is_gone};
use crate::publishing::{
    UnreadableOutlineEvent, outline_in, when_attached, when_detached, when_section_removed,
};

const NAME: &str = "index-attachments";

pub struct AttachmentIndexProjector {
    events: EventSourcingService<Outline>,
    catalog: Arc<dyn OutlineCatalog>,
    outlines: OutlineService,
}

#[derive(Debug, Error)]
enum NotIndexed {
    #[error(transparent)]
    Unreadable(#[from] UnreadableOutlineEvent),
    #[error(transparent)]
    Events(#[from] ServiceError<OutlineError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    NotLetGo(#[from] NotLetGo),
}

impl AttachmentIndexProjector {
    pub fn new(
        store: Arc<dyn EventStore<OutlineEvent>>,
        catalog: Arc<dyn OutlineCatalog>,
        outlines: OutlineService,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: EventSourcingService::new(store, clock),
            catalog,
            outlines,
        }
    }

    async fn index(&self, outline: &OutlineId) -> Result<(), NotIndexed> {
        let standing = self.events.latest(&AggregateId::from(outline)).await?;
        if standing.state.is_discarded() {
            self.catalog.holds(*outline, &[]).await?;
            return Ok(());
        }

        self.catalog
            .holds(*outline, &standing.state.attachments())
            .await?;
        let_go_of_what_is_gone(&self.outlines, self.catalog.as_ref(), *outline).await?;

        Ok(())
    }

    async fn work_through(&self, message: &Message) -> Result<(), NotIndexed> {
        self.index(&outline_in(message)?).await
    }
}

#[async_trait::async_trait]
impl Listener for AttachmentIndexProjector {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the attachment index listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![when_attached(), when_detached(), when_section_removed()]
    }

    fn when_refused(&self) -> &'static str {
        "A change to the outline did not reach the record of what it holds."
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
