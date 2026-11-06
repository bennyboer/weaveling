use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use inbox::KeptHandler;
use messaging::{Failure, Message};
use outbox::{Origin, Outbox};
use outline_contract::{ATTACHED, DETACHED, SECTION_REMOVED};
use outline_core::{KIND, OutlineId, OutlinesCore};
use serde::{Deserialize, Serialize};
use transacting::Transaction;

pub const DETACH_THE_GONE: &str = "follow-up.outline.detach-gone";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct DetachTheGoneDTO {
    pub outline: String,
}

pub struct IndexAttachmentsListener {
    outlines: OutlinesCore,
    outbox: Arc<dyn Outbox>,
    clock: Arc<dyn Clock>,
}

impl IndexAttachmentsListener {
    pub fn new(outlines: OutlinesCore, outbox: Arc<dyn Outbox>, clock: Arc<dyn Clock>) -> Self {
        Self {
            outlines,
            outbox,
            clock,
        }
    }
}

#[async_trait]
impl KeptHandler for IndexAttachmentsListener {
    const NAME: &'static str = "index-attachments";
    const LISTENS_TO: &'static [&'static str] = &[ATTACHED, DETACHED, SECTION_REMOVED];
    const WHEN_REFUSED: &'static str =
        "A change to the outline did not reach the record of what it holds.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let outline = eventpublishing::aggregate_in::<OutlineId>(message)?;

        self.outlines
            .index_attachments(Some(&mut *transaction), &outline)
            .await?;
        self.outbox
            .enqueue_follow_up_within(
                transaction,
                Origin::unversioned(&outline.to_string(), KIND.as_str()),
                message,
                DETACH_THE_GONE,
                &DetachTheGoneDTO {
                    outline: outline.to_string(),
                },
                self.clock.now(),
            )
            .await?;

        Ok(())
    }
}

pub struct DetachGoneAttachmentsListener {
    outlines: OutlinesCore,
}

impl DetachGoneAttachmentsListener {
    pub fn new(outlines: OutlinesCore) -> Self {
        Self { outlines }
    }
}

#[async_trait]
impl KeptHandler for DetachGoneAttachmentsListener {
    const NAME: &'static str = "detach-gone-attachments";
    const LISTENS_TO: &'static [&'static str] = &[DETACH_THE_GONE];
    const WHEN_REFUSED: &'static str =
        "A discarded idea or a deleted scene could not be taken out of the outline.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let asked: DetachTheGoneDTO = message.payload_as()?;

        Ok(self
            .outlines
            .let_go_of_the_gone(Some(transaction), &asked.outline.parse::<OutlineId>()?)
            .await?)
    }
}
