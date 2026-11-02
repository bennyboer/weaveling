use std::sync::Arc;

use async_trait::async_trait;
use boards_core::BoardsCore;
use clock::Clock;
use inbox::KeptHandler;
use messaging::{Failure, Handler, Message, RoutingKey, WithoutInbox};
use outbox::{Origin, Outbox};
use projects_contract::{DELETED, ProjectEventDTO};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use transacting::Transaction;

pub const SWEEP_PROJECT: &str = "follow-up.board.sweep-project";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SweepProjectDTO {
    pub project: String,
}

pub struct RememberDeletedProjectsListener {
    boards: BoardsCore,
    outbox: Arc<dyn Outbox>,
    clock: Arc<dyn Clock>,
}

impl RememberDeletedProjectsListener {
    pub fn new(boards: BoardsCore, outbox: Arc<dyn Outbox>, clock: Arc<dyn Clock>) -> Self {
        Self {
            boards,
            outbox,
            clock,
        }
    }
}

#[async_trait]
impl KeptHandler for RememberDeletedProjectsListener {
    const NAME: &'static str = "boards-remember-deleted-projects";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str =
        "A project's deletion did not reach the boards, so one could still be opened for it.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let deleted = eventpublishing::published_in::<ProjectEventDTO>(message)?;
        let project = deleted.aggregate.id;

        self.boards
            .lay_project_tombstone(Some(&mut *transaction), &project, message.occurred_at)
            .await?;
        self.outbox
            .enqueue_within(
                transaction,
                Origin::unversioned(&project, "project"),
                &sweep_project(message, &project, self.clock.now()),
            )
            .await?;

        Ok(())
    }
}

fn sweep_project(heard: &Message, project: &str, now: OffsetDateTime) -> Message {
    heard.answering(
        RoutingKey::parse(SWEEP_PROJECT).expect("a follow-up's routing key holds no wildcards"),
        json!(SweepProjectDTO {
            project: project.to_owned(),
        }),
        now,
    )
}

pub struct DiscardBoardsOnProjectDeletedListener {
    boards: BoardsCore,
}

impl DiscardBoardsOnProjectDeletedListener {
    pub fn new(boards: BoardsCore) -> Self {
        Self { boards }
    }
}

#[async_trait]
impl Handler for DiscardBoardsOnProjectDeletedListener {
    const NAME: &'static str = "discard-boards-of-deleted-project";
    const LISTENS_TO: &'static [&'static str] = &[SWEEP_PROJECT];
    const WHEN_REFUSED: &'static str = "The board of a deleted project was not cleared away.";
    const WITHOUT_INBOX: WithoutInbox = WithoutInbox::Sweep;

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let asked: SweepProjectDTO = message.payload_as()?;

        Ok(self.boards.sweep(&asked.project).await?)
    }
}
