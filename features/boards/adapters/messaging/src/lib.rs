mod discarding;
mod letting_go;
mod pinning;
mod publishing;
mod sweeping;

pub use discarding::UnpinOnDiscardListener;
pub use pinning::ProjectPinnedIdeasListener;

pub use publishing::message_for;

use eventpublishing::aggregate_in;
use std::sync::Arc;

use async_trait::async_trait;
use boards_contract::{DISCARDED, STARTED};
use boards_core::{Board, BoardCatalog, BoardEvent, BoardId, BoardSummary};
use clock::Clock;
use eventsourcing::{AggregateId, EventSourcingService, EventStore};
use messaging::{Failure, Handler, Message};

pub struct CatalogueBoardsListener {
    events: EventSourcingService<Board>,
    catalog: Arc<dyn BoardCatalog>,
}

impl CatalogueBoardsListener {
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

    async fn catalogue(&self, id: &BoardId) -> Result<(), Failure> {
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
}

#[async_trait]
impl Handler for CatalogueBoardsListener {
    const NAME: &'static str = "catalogue-board";
    const LISTENS_TO: &'static [&'static str] = &[STARTED, DISCARDED];
    const WHEN_REFUSED: &'static str = "A board did not reach the list of boards.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        self.catalogue(&aggregate_in::<BoardId>(message)?).await
    }
}

pub use sweeping::DiscardBoardsOnProjectDeletedListener;
