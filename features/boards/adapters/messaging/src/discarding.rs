use std::sync::Arc;

use boards_core::{
    BoardCatalog, BoardError, BoardService, BoardServiceError, CatalogError, IdeaLink,
};
use eventpublishing::{UnreadableMessage, published_in};
use eventsourcing::{Agent, ServiceError};
use ideas_contract::{DISCARDED, IdeaEventDTO};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use thiserror::Error;

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
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Boards(#[from] BoardServiceError),
}

pub fn when_discarded() -> Subscription {
    Subscription::parse(DISCARDED).expect("a declared routing key holds no wildcards")
}

impl UnpinOnDiscard {
    pub fn new(boards: BoardService, catalog: Arc<dyn BoardCatalog>) -> Self {
        Self { boards, catalog }
    }

    async fn unpin_everywhere(&self, idea: &IdeaLink) -> Result<(), NotUnpinned> {
        for board in self.catalog.boards_holding(idea).await? {
            match self
                .boards
                .unpin(&board.to_string(), idea.clone(), None, &nobody())
                .await
            {
                Ok(_) => {}
                Err(refused) if nothing_left_to_unpin(&refused) => {}
                Err(refused) => return Err(refused.into()),
            }
        }

        Ok(())
    }

    async fn work_through(&self, message: &Message) -> Result<(), NotUnpinned> {
        let discarded = published_in::<IdeaEventDTO>(message)?;

        self.unpin_everywhere(&IdeaLink::from(discarded.aggregate.id.as_str()))
            .await
    }
}

fn nothing_left_to_unpin(refused: &BoardServiceError) -> bool {
    matches!(
        refused,
        BoardServiceError::Events(ServiceError::Refused(
            BoardError::NotPinned | BoardError::Discarded
        ))
    )
}

fn nobody() -> Agent {
    Agent::System
}

#[async_trait::async_trait]
impl Listener for UnpinOnDiscard {
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
