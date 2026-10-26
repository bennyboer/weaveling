use boards_contract::{IDEA_PINNED, IDEA_UNPINNED};
use boards_core::{BoardId, BoardsCore};
use messaging::{Failure, Handler, Message};

pub struct ProjectPinnedIdeasListener {
    boards: BoardsCore,
}

impl ProjectPinnedIdeasListener {
    pub fn new(boards: BoardsCore) -> Self {
        Self { boards }
    }
}

#[async_trait::async_trait]
impl Handler for ProjectPinnedIdeasListener {
    const NAME: &'static str = "index-pinned-ideas";
    const LISTENS_TO: &'static [&'static str] = &[IDEA_PINNED, IDEA_UNPINNED];
    const WHEN_REFUSED: &'static str =
        "A pin did not reach the record of which ideas a board holds.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        Ok(self
            .boards
            .index_pins(&eventpublishing::aggregate_in::<BoardId>(message)?)
            .await?)
    }
}
