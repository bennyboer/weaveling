use std::sync::Arc;

use boards_core::{BoardCatalog, BoardService, IdeaLink};
use eventpublishing::published_in;
use ideas_contract::{DISCARDED, IdeaEventDTO};
use messaging::{Failure, Handler, Message};

use crate::letting_go::let_go_everywhere;

pub struct UnpinOnDiscard {
    boards: BoardService,
    catalog: Arc<dyn BoardCatalog>,
}

impl UnpinOnDiscard {
    pub fn new(boards: BoardService, catalog: Arc<dyn BoardCatalog>) -> Self {
        Self { boards, catalog }
    }
}

#[async_trait::async_trait]
impl Handler for UnpinOnDiscard {
    const NAME: &'static str = "unpin-discarded-idea";
    const LISTENS_TO: &'static [&'static str] = &[DISCARDED];
    const WHEN_REFUSED: &'static str = "A discarded idea could not be taken off a board.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let discarded = published_in::<IdeaEventDTO>(message)?;

        let_go_everywhere(
            &self.boards,
            self.catalog.as_ref(),
            &IdeaLink::from(discarded.aggregate.id.as_str()),
        )
        .await?;

        Ok(())
    }
}
