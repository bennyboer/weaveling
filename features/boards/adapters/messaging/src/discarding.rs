use std::sync::Arc;

use boards_core::{BoardCatalog, BoardService, BoardServiceError, CatalogError, PieceLink};
use eventpublishing::{UnreadableMessage, published_in};
use eventsourcing::Agent;
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use pieces_contract::{DISCARDED, PieceEventDTO};
use thiserror::Error;

const NAME: &str = "unpin-discarded-piece";

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

    async fn unpin_everywhere(&self, piece: &PieceLink) -> Result<(), NotUnpinned> {
        for board in self.catalog.boards_holding(piece).await? {
            self.boards
                .unpin(&board.to_string(), piece.clone(), None, &nobody())
                .await?;
        }

        Ok(())
    }

    async fn work_through(&self, message: &Message) -> Result<(), NotUnpinned> {
        let discarded = published_in::<PieceEventDTO>(message)?;

        self.unpin_everywhere(&PieceLink::from(discarded.aggregate.id.as_str()))
            .await
    }
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
