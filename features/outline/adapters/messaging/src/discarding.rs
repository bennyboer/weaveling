use std::sync::Arc;

use eventpublishing::{UnreadableMessage, published_in};
use eventsourcing::{Agent, ServiceError};
use ideas_contract::{DISCARDED, IdeaEventDTO};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use outline_core::{
    Attachment, CatalogError, OutlineCatalog, OutlineError, OutlineService, OutlineServiceError,
};
use thiserror::Error;

const NAME: &str = "detach-discarded-idea";

pub struct DetachOnDiscard {
    outlines: OutlineService,
    catalog: Arc<dyn OutlineCatalog>,
}

#[derive(Debug, Error)]
enum NotDetached {
    #[error(transparent)]
    Unreadable(#[from] UnreadableMessage),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Outlines(#[from] OutlineServiceError),
}

pub fn when_discarded() -> Subscription {
    Subscription::parse(DISCARDED).expect("a declared routing key holds no wildcards")
}

impl DetachOnDiscard {
    pub fn new(outlines: OutlineService, catalog: Arc<dyn OutlineCatalog>) -> Self {
        Self { outlines, catalog }
    }

    async fn detach_everywhere(&self, note: &Attachment) -> Result<(), NotDetached> {
        for outline in self.catalog.outlines_holding(note).await? {
            match self
                .outlines
                .detach(&outline.to_string(), note.clone(), None, &nobody())
                .await
            {
                Ok(_) => {}
                Err(refused) if already_detached(&refused) => {}
                Err(refused) => return Err(refused.into()),
            }
        }

        Ok(())
    }

    async fn work_through(&self, message: &Message) -> Result<(), NotDetached> {
        let discarded = published_in::<IdeaEventDTO>(message)?;

        self.detach_everywhere(&Attachment::idea(&discarded.aggregate.id))
            .await
    }
}

fn already_detached(refused: &OutlineServiceError) -> bool {
    matches!(
        refused,
        OutlineServiceError::Events(ServiceError::Refused(OutlineError::NotAttached))
    )
}

fn nobody() -> Agent {
    Agent::System
}

#[async_trait::async_trait]
impl Listener for DetachOnDiscard {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the discard listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![when_discarded()]
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
