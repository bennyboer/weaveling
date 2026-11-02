use ideas_contract::{DISCARDED, IdeaEventDTO};
use inbox::{KeptHandler, Refused};
use messaging::Message;
use scenes_core::ScenesCore;
use transacting::Transaction;

pub struct UnlinkOnDiscardListener {
    scenes: ScenesCore,
}

impl UnlinkOnDiscardListener {
    pub fn new(scenes: ScenesCore) -> Self {
        Self { scenes }
    }
}

#[async_trait::async_trait]
impl KeptHandler for UnlinkOnDiscardListener {
    const NAME: &'static str = "unlink-discarded-idea";
    const LISTENS_TO: &'static [&'static str] = &[DISCARDED];
    const WHEN_REFUSED: &'static str = "A discarded idea could not be unlinked from its scenes.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Refused> {
        let discarded = eventpublishing::published_in::<IdeaEventDTO>(message)?;

        Ok(self
            .scenes
            .unlink_everywhere(Some(transaction), &discarded.aggregate.id)
            .await?)
    }
}
