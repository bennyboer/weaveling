use inbox::KeptHandler;
use messaging::{Failure, Message};
use outline_contract::{OutlineStateDTO, STATE_CHANGED};
use readings_core::ReadingsCore;
use transacting::Transaction;

use crate::transformer;

pub struct KeepOutlinesListener {
    readings: ReadingsCore,
}

impl KeepOutlinesListener {
    pub fn new(readings: ReadingsCore) -> Self {
        Self { readings }
    }
}

#[async_trait::async_trait]
impl KeptHandler for KeepOutlinesListener {
    const NAME: &'static str = "readings-keep-outlines";
    const LISTENS_TO: &'static [&'static str] = &[STATE_CHANGED];
    const WHEN_REFUSED: &'static str = "A book's outline did not reach the readings feature. The book may show its sections as they were before.";

    /// Keeps the outline as it was told, or forgets the book once its outline is discarded.
    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told: OutlineStateDTO = message.payload_as()?;
        if told.discarded {
            return Ok(self
                .readings
                .book_gone(Some(transaction), &told.project, message.occurred_at)
                .await?);
        }

        Ok(self
            .readings
            .outline_changed(
                Some(transaction),
                &told.project,
                &transformer::to_told_sections(&told.sections),
                told.version,
            )
            .await?)
    }
}
