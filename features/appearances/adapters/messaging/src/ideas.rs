use appearances_core::AppearancesCore;
use ideas_contract::{CAPTURED, DISCARDED, IdeaEventDTO};
use inbox::KeptHandler;
use messaging::{Failure, Message};
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
    ) -> Result<(), Failure> {
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

pub struct RememberIdeaProjectListener {
    appearances: AppearancesCore,
}

impl RememberIdeaProjectListener {
    pub fn new(appearances: AppearancesCore) -> Self {
        Self { appearances }
    }
}

#[async_trait::async_trait]
impl KeptHandler for RememberIdeaProjectListener {
    const NAME: &'static str = "remember-project-of-idea";
    const LISTENS_TO: &'static [&'static str] = &[CAPTURED];
    const WHEN_REFUSED: &'static str = "Where an idea appears is not shown.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let captured = eventpublishing::published_in::<IdeaEventDTO>(message)?;
        let IdeaEventDTO::Captured { project, .. } = captured.event.body else {
            return Ok(());
        };

        Ok(self
            .appearances
            .captured(Some(transaction), &captured.aggregate.id, &project)
            .await?)
    }
}
