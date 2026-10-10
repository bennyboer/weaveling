use std::sync::Arc;

use boards_core::{Board, BoardCatalog, BoardError, BoardEvent, BoardId, CatalogError, IdeaLink};
use clock::Clock;
use eventsourcing::{AggregateId, EventSourcingService, EventStore, ServiceError};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use thiserror::Error;

use crate::publishing::{UnreadableBoardEvent, board_in, when_pinned, when_unpinned};

const NAME: &str = "index-pinned-ideas";

pub struct PinnedIdeasProjector {
    events: EventSourcingService<Board>,
    catalog: Arc<dyn BoardCatalog>,
}

#[derive(Debug, Error)]
enum NotIndexed {
    #[error(transparent)]
    Unreadable(#[from] UnreadableBoardEvent),
    #[error(transparent)]
    Events(#[from] ServiceError<BoardError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

impl PinnedIdeasProjector {
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

    async fn index(&self, board: &BoardId) -> Result<(), NotIndexed> {
        let standing = self.events.latest(&AggregateId::from(board)).await?;
        let holding: Vec<IdeaLink> = if standing.state.is_discarded() {
            Vec::new()
        } else {
            standing
                .state
                .ideas()
                .into_iter()
                .map(|positioned| positioned.idea)
                .collect()
        };

        self.catalog.holds(*board, &holding).await?;

        Ok(())
    }

    async fn work_through(&self, message: &Message) -> Result<(), NotIndexed> {
        self.index(&board_in(message)?).await
    }
}

#[async_trait::async_trait]
impl Listener for PinnedIdeasProjector {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the pin index listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![when_pinned(), when_unpinned()]
    }

    fn when_refused(&self) -> &'static str {
        "A pin did not reach the record of which ideas a board holds."
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
