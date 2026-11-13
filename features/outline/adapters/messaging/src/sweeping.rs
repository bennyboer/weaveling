use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use inbox::KeptHandler;
use messaging::{Failure, Handler, Message, WithoutInbox};
use outbox::{Origin, Outbox};
use outline_core::OutlinesCore;
use projects_contract::{DELETED, ProjectEventDTO};
use serde::{Deserialize, Serialize};
use transacting::Transaction;

pub const SWEEP_PROJECT: &str = "follow-up.outline.sweep-project";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SweepProjectDTO {
    pub project: String,
}

pub struct RememberDeletedProjectsListener {
    outlines: OutlinesCore,
    outbox: Arc<dyn Outbox>,
    clock: Arc<dyn Clock>,
}

impl RememberDeletedProjectsListener {
    pub fn new(outlines: OutlinesCore, outbox: Arc<dyn Outbox>, clock: Arc<dyn Clock>) -> Self {
        Self {
            outlines,
            outbox,
            clock,
        }
    }
}

#[async_trait]
impl KeptHandler for RememberDeletedProjectsListener {
    const NAME: &'static str = "outline-remember-deleted-projects";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str = "The outline feature did not learn that a project was deleted. So an outline could still be opened for that project.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let deleted = eventpublishing::published_in::<ProjectEventDTO>(message)?;
        let project = deleted.aggregate.id;

        self.outlines
            .lay_project_tombstone(Some(&mut *transaction), &project, message.occurred_at)
            .await?;
        self.outbox
            .enqueue_follow_up_within(
                transaction,
                Origin::unversioned(&project, "project"),
                message,
                SWEEP_PROJECT,
                &SweepProjectDTO {
                    project: project.clone(),
                },
                self.clock.now(),
            )
            .await?;

        Ok(())
    }
}

pub struct DiscardOutlinesOnProjectDeletedListener {
    outlines: OutlinesCore,
}

impl DiscardOutlinesOnProjectDeletedListener {
    pub fn new(outlines: OutlinesCore) -> Self {
        Self { outlines }
    }
}

#[async_trait]
impl Handler for DiscardOutlinesOnProjectDeletedListener {
    const NAME: &'static str = "discard-outlines-of-deleted-project";
    const LISTENS_TO: &'static [&'static str] = &[SWEEP_PROJECT];
    const WHEN_REFUSED: &'static str = "The outline of a deleted project was not removed.";
    const WITHOUT_INBOX: WithoutInbox = WithoutInbox::Sweep;

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let asked: SweepProjectDTO = message.payload_as()?;

        Ok(self.outlines.sweep(&asked.project).await?)
    }
}
