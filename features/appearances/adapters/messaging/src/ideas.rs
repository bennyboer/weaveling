use std::sync::Arc;

use appearances_core::{AppearanceCatalog, CatalogError, IdeaLink, Subject};
use eventpublishing::{UnreadableMessage, published_in};
use ideas_contract::{DISCARDED, IdeaEventDTO};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use thiserror::Error;

const NAME: &str = "forget-appearances-of-discarded-idea";

pub struct ForgetDiscardedIdea {
    catalog: Arc<dyn AppearanceCatalog>,
}

#[derive(Debug, Error)]
enum NotForgotten {
    #[error(transparent)]
    Unreadable(#[from] UnreadableMessage),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

pub fn when_idea_discarded() -> Subscription {
    Subscription::parse(DISCARDED).expect("a declared routing key holds no wildcards")
}

impl ForgetDiscardedIdea {
    pub fn new(catalog: Arc<dyn AppearanceCatalog>) -> Self {
        Self { catalog }
    }

    async fn forget(&self, message: &Message) -> Result<(), NotForgotten> {
        let discarded = published_in::<IdeaEventDTO>(message)?;

        Ok(self
            .catalog
            .forget_subject(&Subject::Idea(IdeaLink::from(discarded.aggregate.id)))
            .await?)
    }
}

#[async_trait::async_trait]
impl Listener for ForgetDiscardedIdea {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the discard listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![when_idea_discarded()]
    }

    fn delivery(&self) -> Delivery {
        Delivery::Kept
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        self.forget(message)
            .await
            .map_err(|why| NotHandled::because(self.named(), message.routing.clone(), why))
    }
}
