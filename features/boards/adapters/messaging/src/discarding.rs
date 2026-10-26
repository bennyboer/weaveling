use boards_core::BoardsCore;
use ideas_contract::{DISCARDED, IdeaEventDTO};
use messaging::{Failure, Handler, Message};

pub struct UnpinOnDiscardListener {
    boards: BoardsCore,
}

impl UnpinOnDiscardListener {
    pub fn new(boards: BoardsCore) -> Self {
        Self { boards }
    }
}

#[async_trait::async_trait]
impl Handler for UnpinOnDiscardListener {
    const NAME: &'static str = "unpin-discarded-idea";
    const LISTENS_TO: &'static [&'static str] = &[DISCARDED];
    const WHEN_REFUSED: &'static str = "A discarded idea could not be taken off a board.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let discarded = eventpublishing::published_in::<IdeaEventDTO>(message)?;

        Ok(self
            .boards
            .let_go_everywhere(&discarded.aggregate.id, message.occurred_at)
            .await?)
    }
}
