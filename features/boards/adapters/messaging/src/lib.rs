mod access;
mod discarding;
mod pinning;
mod publishing;
mod sweeping;

pub use access::{RememberRolesListener, RememberTrashListener};
pub use discarding::{
    LET_GO_EVERYWHERE, LetGoEverywhereDTO, LetGoOfDiscardedIdeaListener,
    RememberProjectsOfIdeasListener, UnpinOnDiscardListener,
};
pub use pinning::{
    LET_GO_OF_THE_GONE, LetGoOfGoneIdeasListener, LetGoOfTheGoneDTO, ProjectPinnedIdeasListener,
};

use async_trait::async_trait;
use boards_contract::{DISCARDED, STARTED};
use boards_core::{BoardId, BoardsCore};
use inbox::KeptHandler;
use messaging::{Failure, Message};
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
    ) -> Result<(), Failure> {
        Ok(self
            .boards
            .catalogue(
                Some(transaction),
                &eventpublishing::aggregate_in::<BoardId>(message)?,
            )
            .await?)
    }
}

pub use sweeping::{
    DiscardBoardsOnProjectDeletedListener, RememberDeletedProjectsListener, SWEEP_PROJECT,
    SweepProjectDTO,
};
