use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use inbox::KeptHandler;
use messaging::{Failure, Handler, Message, WithoutInbox};
use outbox::{Origin, Outbox};
use projects_contract::{DELETED, ProjectEventDTO};
use readings_core::ReadingsCore;
use serde::{Deserialize, Serialize};
use transacting::Transaction;

pub const SWEEP_PROJECT: &str = "follow-up.reading-copy.sweep-project";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SweepProjectDTO {
    pub project: String,
}

/// Asks, once a project is deleted, for its reading copies to be swept.
/// The sweep follows on its own, and deletes each copy in a transaction of its own.
/// It is not strictly needed here: the copies live in the feature's own database, so this listener could delete them all in its own transaction.
/// It is kept like this for two reasons.
/// The other features sweep a deleted project's pieces the same way.
/// And a project with a great many copies does not make one transaction that large.
pub struct SweepCopiesOfDeletedProjectListener {
    outbox: Arc<dyn Outbox>,
    clock: Arc<dyn Clock>,
}

impl SweepCopiesOfDeletedProjectListener {
    pub fn new(outbox: Arc<dyn Outbox>, clock: Arc<dyn Clock>) -> Self {
        Self { outbox, clock }
    }
}

#[async_trait]
impl KeptHandler for SweepCopiesOfDeletedProjectListener {
    const NAME: &'static str = "readings-sweep-copies-of-deleted-project";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str = "The readings feature did not learn that a project was deleted. Its reading copies stay stored until each one's day of deletion, or for good.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let deleted = eventpublishing::published_in::<ProjectEventDTO>(message)?;
        let project = deleted.aggregate.id;

        self.outbox
            .enqueue_follow_up_within(
                transaction,
                Origin {
                    aggregate: &project,
                    kind: "project",
                    version: Some(deleted.aggregate.version),
                },
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

/// Deletes every reading copy of a deleted project, with what each holds.
pub struct DeleteCopiesOfDeletedProjectListener {
    readings: ReadingsCore,
}

impl DeleteCopiesOfDeletedProjectListener {
    pub fn new(readings: ReadingsCore) -> Self {
        Self { readings }
    }
}

#[async_trait]
impl Handler for DeleteCopiesOfDeletedProjectListener {
    const NAME: &'static str = "delete-reading-copies-of-deleted-project";
    const LISTENS_TO: &'static [&'static str] = &[SWEEP_PROJECT];
    const WHEN_REFUSED: &'static str = "The reading copies of a deleted project were not deleted. They stay stored until each one's day of deletion, or for good.";
    const WITHOUT_INBOX: WithoutInbox = WithoutInbox::Sweep;

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let asked: SweepProjectDTO = message.payload_as()?;

        Ok(self.readings.sweep_project(&asked.project).await?)
    }
}
