mod discarding;
mod pinning;
mod publishing;
mod sweeping;

pub use discarding::UnpinOnDiscardListener;
pub use pinning::ProjectPinnedIdeasListener;

use async_trait::async_trait;
use boards_contract::{DISCARDED, STARTED};
use boards_core::{BoardId, BoardsCore};
use inbox::{KeptHandler, Refused};
use messaging::Message;
pub use publishing::message_for;
use transacting::Transaction;

pub struct CatalogueBoardsListener {
    boards: BoardsCore,
}

impl CatalogueBoardsListener {
    pub fn new(boards: BoardsCore) -> Self {
        Self { boards }
    }
}

#[async_trait]
impl KeptHandler for CatalogueBoardsListener {
    const NAME: &'static str = "catalogue-board";
    const LISTENS_TO: &'static [&'static str] = &[STARTED, DISCARDED];
    const WHEN_REFUSED: &'static str = "A board did not reach the list of boards.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Refused> {
        self.boards
            .catalogue(
                None,
                &eventpublishing::aggregate_in::<BoardId>(message)?,
            )
            .await
            .map_err(Refused::of)
    }
}

pub use sweeping::DiscardBoardsOnProjectDeletedListener;
