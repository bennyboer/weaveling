use std::sync::Arc;

use boards_core::{BoardCatalog, BoardService, IdeaLink};
use eventpublishing::{UnreadableMessage, published_in};
use ideas_contract::{DISCARDED, IdeaEventDTO};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use thiserror::Error;

use crate::letting_go::{NotLetGo, let_go_everywhere};

const NAME: &str = "unpin-discarded-idea";

pub struct UnpinOnDiscard {
    boards: BoardService,
    catalog: Arc<dyn BoardCatalog>,
}

#[derive(Debug, Error)]
enum NotUnpinned {
    #[error(transparent)]
    Unreadable(#[from] UnreadableMessage),
    #[error(transparent)]
    NotLetGo(#[from] NotLetGo),
}

pub fn when_discarded() -> Subscription {
    Subscription::parse(DISCARDED).expect("a declared routing key holds no wildcards")
}

impl UnpinOnDiscard {
    pub fn new(boards: BoardService, catalog: Arc<dyn BoardCatalog>) -> Self {
        Self { boards, catalog }
    }

    async fn work_through(&self, message: &Message) -> Result<(), NotUnpinned> {
        let discarded = published_in::<IdeaEventDTO>(message)?;

        let_go_everywhere(
            &self.boards,
            self.catalog.as_ref(),
            &IdeaLink::from(discarded.aggregate.id.as_str()),
        )
        .await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl Listener for UnpinOnDiscard {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the discard listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![when_discarded()]
    }

    fn when_refused(&self) -> &'static str {
        "A discarded idea could not be taken off a board."
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
