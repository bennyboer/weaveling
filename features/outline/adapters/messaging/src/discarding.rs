use std::sync::Arc;

use eventpublishing::{UnreadableMessage, published_in};
use ideas_contract::{DISCARDED, IdeaEventDTO};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use outline_core::{Attachment, OutlineCatalog, OutlineService};
use thiserror::Error;

use crate::letting_go::{NotLetGo, let_go_everywhere};

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
    NotLetGo(#[from] NotLetGo),
}

pub fn when_discarded() -> Subscription {
    Subscription::parse(DISCARDED).expect("a declared routing key holds no wildcards")
}

impl DetachOnDiscard {
    pub fn new(outlines: OutlineService, catalog: Arc<dyn OutlineCatalog>) -> Self {
        Self { outlines, catalog }
    }

    async fn work_through(&self, message: &Message) -> Result<(), NotDetached> {
        let discarded = published_in::<IdeaEventDTO>(message)?;

        let_go_everywhere(
            &self.outlines,
            self.catalog.as_ref(),
            &Attachment::idea(&discarded.aggregate.id),
        )
        .await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl Listener for DetachOnDiscard {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the discard listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![when_discarded()]
    }

    fn when_refused(&self) -> &'static str {
        "A discarded idea could not be taken out of the outline."
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
