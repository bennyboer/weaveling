use appearances_core::AppearancesCore;
use ideas_contract::{DISCARDED, IdeaEventDTO};
use inbox::{KeptHandler, Refused};
use messaging::Message;
use transacting::Transaction;

pub struct ForgetDiscardedIdeaListener {
    appearances: AppearancesCore,
}

impl ForgetDiscardedIdeaListener {
    pub fn new(appearances: AppearancesCore) -> Self {
        Self { appearances }
    }
}

#[async_trait::async_trait]
impl KeptHandler for ForgetDiscardedIdeaListener {
    const NAME: &'static str = "forget-appearances-of-discarded-idea";
    const LISTENS_TO: &'static [&'static str] = &[DISCARDED];
    const WHEN_REFUSED: &'static str = "A discarded idea is still listed where it appeared.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Refused> {
        let discarded = eventpublishing::published_in::<IdeaEventDTO>(message)?;

        Ok(self
            .appearances
            .idea_discarded(
                Some(transaction),
                &discarded.aggregate.id,
                message.occurred_at,
            )
            .await?)
    }
}
