use std::sync::Arc;

use async_trait::async_trait;
use boards_core::BoardsCore;
use clock::Clock;
use ideas_contract::{DISCARDED, IdeaEventDTO};
use inbox::KeptHandler;
use messaging::{Failure, Message, RoutingKey};
use outbox::{Origin, Outbox};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use transacting::Transaction;

pub const LET_GO_EVERYWHERE: &str = "follow-up.board.let-go-everywhere";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct LetGoEverywhereDTO {
    pub idea: String,
}

pub struct UnpinOnDiscardListener {
    boards: BoardsCore,
    outbox: Arc<dyn Outbox>,
    clock: Arc<dyn Clock>,
}

impl UnpinOnDiscardListener {
    pub fn new(boards: BoardsCore, outbox: Arc<dyn Outbox>, clock: Arc<dyn Clock>) -> Self {
        Self {
            boards,
            outbox,
            clock,
        }
    }
}

#[async_trait]
impl KeptHandler for UnpinOnDiscardListener {
    const NAME: &'static str = "unpin-discarded-idea";
    const LISTENS_TO: &'static [&'static str] = &[DISCARDED];
    const WHEN_REFUSED: &'static str = "A discarded idea could not be taken off a board.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let discarded = eventpublishing::published_in::<IdeaEventDTO>(message)?;
        let idea = discarded.aggregate.id;

        self.boards
            .lay_tombstone(Some(&mut *transaction), &idea, message.occurred_at)
            .await?;
        self.outbox
            .enqueue_within(
                transaction,
                Origin {
                    aggregate: &idea,
                    kind: "idea",
                    version: 0,
                },
                &let_go_everywhere(message, &idea, self.clock.now()),
            )
            .await?;

        Ok(())
    }
}

fn let_go_everywhere(heard: &Message, idea: &str, now: OffsetDateTime) -> Message {
    heard.answering(
        RoutingKey::parse(LET_GO_EVERYWHERE).expect("a follow-up's routing key holds no wildcards"),
        json!(LetGoEverywhereDTO {
            idea: idea.to_owned(),
        }),
        now,
    )
}

pub struct LetGoOfDiscardedIdeaListener {
    boards: BoardsCore,
}

impl LetGoOfDiscardedIdeaListener {
    pub fn new(boards: BoardsCore) -> Self {
        Self { boards }
    }
}

#[async_trait]
impl KeptHandler for LetGoOfDiscardedIdeaListener {
    const NAME: &'static str = "let-go-of-discarded-idea-everywhere";
    const LISTENS_TO: &'static [&'static str] = &[LET_GO_EVERYWHERE];
    const WHEN_REFUSED: &'static str = "A discarded idea could not be taken off a board.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let asked: LetGoEverywhereDTO = message.payload_as()?;

        Ok(self
            .boards
            .let_go_everywhere(Some(transaction), &asked.idea)
            .await?)
    }
}
