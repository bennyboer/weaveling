use std::sync::Arc;

use async_trait::async_trait;
use boards_contract::{IDEA_PINNED, IDEA_UNPINNED};
use boards_core::{BoardId, BoardsCore, KIND};
use clock::Clock;
use inbox::KeptHandler;
use messaging::{Failure, Message, RoutingKey};
use outbox::{Origin, Outbox};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use transacting::Transaction;

pub const LET_GO_OF_THE_GONE: &str = "follow-up.board.let-go-gone";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct LetGoOfTheGoneDTO {
    pub board: String,
}

pub struct ProjectPinnedIdeasListener {
    boards: BoardsCore,
    outbox: Arc<dyn Outbox>,
    clock: Arc<dyn Clock>,
}

impl ProjectPinnedIdeasListener {
    pub fn new(boards: BoardsCore, outbox: Arc<dyn Outbox>, clock: Arc<dyn Clock>) -> Self {
        Self {
            boards,
            outbox,
            clock,
        }
    }
}

#[async_trait]
impl KeptHandler for ProjectPinnedIdeasListener {
    const NAME: &'static str = "index-pinned-ideas";
    const LISTENS_TO: &'static [&'static str] = &[IDEA_PINNED, IDEA_UNPINNED];
    const WHEN_REFUSED: &'static str =
        "A pin did not reach the record of which ideas a board holds.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let board = eventpublishing::aggregate_in::<BoardId>(message)?;

        self.boards
            .index_pins(Some(&mut *transaction), &board)
            .await?;
        self.outbox
            .enqueue_within(
                transaction,
                Origin {
                    aggregate: &board.to_string(),
                    kind: KIND.as_str(),
                    version: 0,
                },
                &let_go_of_the_gone(message, &board, self.clock.now()),
            )
            .await?;

        Ok(())
    }
}

fn let_go_of_the_gone(heard: &Message, board: &BoardId, now: OffsetDateTime) -> Message {
    heard.answering(
        RoutingKey::parse(LET_GO_OF_THE_GONE)
            .expect("a follow-up's routing key holds no wildcards"),
        json!(LetGoOfTheGoneDTO {
            board: board.to_string(),
        }),
        now,
    )
}

pub struct LetGoOfGoneIdeasListener {
    boards: BoardsCore,
}

impl LetGoOfGoneIdeasListener {
    pub fn new(boards: BoardsCore) -> Self {
        Self { boards }
    }
}

#[async_trait]
impl KeptHandler for LetGoOfGoneIdeasListener {
    const NAME: &'static str = "let-go-of-gone-ideas";
    const LISTENS_TO: &'static [&'static str] = &[LET_GO_OF_THE_GONE];
    const WHEN_REFUSED: &'static str = "A discarded idea could not be taken off a board.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let asked: LetGoOfTheGoneDTO = message.payload_as()?;

        Ok(self
            .boards
            .let_go_of_the_gone(Some(transaction), &asked.board.parse::<BoardId>()?)
            .await?)
    }
}
