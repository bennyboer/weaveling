mod discarding;
mod pinning;
mod publishing;
mod sweeping;

pub use discarding::UnpinOnDiscardListener;
pub use pinning::ProjectPinnedIdeasListener;

use async_trait::async_trait;
use boards_contract::{DISCARDED, STARTED};
use boards_core::{BoardId, BoardsCore};
use messaging::{Failure, Handler, Message};
pub use publishing::message_for;

pub struct CatalogueBoardsListener {
    boards: BoardsCore,
}

impl CatalogueBoardsListener {
    pub fn new(boards: BoardsCore) -> Self {
        Self { boards }
    }
}

#[async_trait]
impl Handler for CatalogueBoardsListener {
    const NAME: &'static str = "catalogue-board";
    const LISTENS_TO: &'static [&'static str] = &[STARTED, DISCARDED];
    const WHEN_REFUSED: &'static str = "A board did not reach the list of boards.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        Ok(self
            .boards
            .catalogue(&eventpublishing::aggregate_in::<BoardId>(message)?)
            .await?)
    }
}

pub use sweeping::DiscardBoardsOnProjectDeletedListener;
