use boards_core::BoardsCore;
use messaging::{Failure, Handler, Message};
use projects_contract::{DELETED, ProjectEventDTO};

pub struct DiscardBoardsOnProjectDeletedListener {
    boards: BoardsCore,
}

impl DiscardBoardsOnProjectDeletedListener {
    pub fn new(boards: BoardsCore) -> Self {
        Self { boards }
    }
}

#[async_trait::async_trait]
impl Handler for DiscardBoardsOnProjectDeletedListener {
    const NAME: &'static str = "discard-boards-of-deleted-project";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str = "The board of a deleted project was not cleared away.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let deleted = eventpublishing::published_in::<ProjectEventDTO>(message)?;

        Ok(self.boards.sweep(&deleted.aggregate.id).await?)
    }
}
