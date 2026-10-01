mod discarding;
mod pinning;
mod publishing;
mod sweeping;

pub use discarding::{UnpinOnDiscard, when_discarded};
pub use pinning::PinnedIdeasProjector;

pub use publishing::{
    BoardEventPublisher, UnreadableBoardEvent, board_in, event_in, every_event, message_for,
    when_board_discarded, when_pinned, when_started, when_unpinned,
};

use std::sync::Arc;

use async_trait::async_trait;
use boards_core::{
    Board, BoardCatalog, BoardError, BoardEvent, BoardId, BoardSummary, CatalogError,
};
use clock::Clock;
use eventsourcing::{AggregateId, EventSourcingService, EventStore, ServiceError};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use thiserror::Error;

const NAME: &str = "catalogue-board";

pub struct BoardCatalogProjector {
    events: EventSourcingService<Board>,
    catalog: Arc<dyn BoardCatalog>,
}

#[derive(Debug, Error)]
enum NotCatalogued {
    #[error(transparent)]
    Unreadable(#[from] UnreadableBoardEvent),
    #[error(transparent)]
    Events(#[from] ServiceError<BoardError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

impl BoardCatalogProjector {
    pub fn new(
        store: Arc<dyn EventStore<BoardEvent>>,
        catalog: Arc<dyn BoardCatalog>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: EventSourcingService::new(store, clock),
            catalog,
        }
    }

    async fn catalogue(&self, id: &BoardId) -> Result<(), NotCatalogued> {
        let standing = self.events.latest(&AggregateId::from(id)).await?;

        if standing.state.is_discarded() {
            self.catalog.forget(id).await?;
        } else {
            self.catalog
                .remember(&BoardSummary::of(*id, &standing.state))
                .await?;
        }

        Ok(())
    }

    async fn work_through(&self, message: &Message) -> Result<(), NotCatalogued> {
        self.catalogue(&board_in(message)?).await
    }
}

#[async_trait]
impl Listener for BoardCatalogProjector {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the catalog listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![when_started(), when_board_discarded()]
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

pub use sweeping::{DiscardBoardsOnProjectDeleted, when_project_deleted};
