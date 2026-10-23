use std::sync::Arc;

use boards_contract::{IDEA_PINNED, IDEA_UNPINNED};
use boards_core::{Board, BoardCatalog, BoardEvent, BoardId, BoardService, IdeaLink};
use clock::Clock;
use eventpublishing::aggregate_in;
use eventsourcing::{AggregateId, EventSourcingService, EventStore};
use messaging::{Failure, Handler, Message};

use crate::letting_go;

pub struct ProjectPinnedIdeasListener {
    events: EventSourcingService<Board>,
    catalog: Arc<dyn BoardCatalog>,
    boards: BoardService,
}

impl ProjectPinnedIdeasListener {
    pub fn new(
        store: Arc<dyn EventStore<BoardEvent>>,
        catalog: Arc<dyn BoardCatalog>,
        boards: BoardService,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: EventSourcingService::new(store, clock),
            catalog,
            boards,
        }
    }

    async fn index(&self, board: &BoardId) -> Result<(), Failure> {
        let standing = self.events.latest(&AggregateId::from(board)).await?;
        if standing.state.is_discarded() {
            self.catalog.holds(*board, &[]).await?;
            return Ok(());
        }

        let holding: Vec<IdeaLink> = standing
            .state
            .ideas()
            .into_iter()
            .map(|positioned| positioned.idea)
            .collect();
        self.catalog.holds(*board, &holding).await?;
        letting_go::let_go_of_what_is_gone(&self.boards, self.catalog.as_ref(), *board).await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl Handler for ProjectPinnedIdeasListener {
    const NAME: &'static str = "index-pinned-ideas";
    const LISTENS_TO: &'static [&'static str] = &[IDEA_PINNED, IDEA_UNPINNED];
    const WHEN_REFUSED: &'static str =
        "A pin did not reach the record of which ideas a board holds.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        self.index(&aggregate_in::<BoardId>(message)?).await
    }
}
