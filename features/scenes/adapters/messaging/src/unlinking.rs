use ideas_contract::{CAPTURED, DISCARDED, IdeaEventDTO};
use inbox::KeptHandler;
use messaging::{Failure, Message};
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
    ) -> Result<(), Failure> {
        let discarded = eventpublishing::published_in::<IdeaEventDTO>(message)?;

        Ok(self
            .scenes
            .unlink_everywhere(
                Some(transaction),
                &discarded.aggregate.id,
                message.occurred_at,
            )
            .await?)
    }
}

pub struct RememberProjectsOfIdeasListener {
    scenes: ScenesCore,
}

impl RememberProjectsOfIdeasListener {
    pub fn new(scenes: ScenesCore) -> Self {
        Self { scenes }
    }
}

#[async_trait::async_trait]
impl KeptHandler for RememberProjectsOfIdeasListener {
    const NAME: &'static str = "scenes-remember-projects-of-ideas";
    const LISTENS_TO: &'static [&'static str] = &[CAPTURED];
    const WHEN_REFUSED: &'static str = "The scenes feature did not learn which project a new idea belongs to. So a scene of a different project could link this idea.";

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
            .scenes
            .idea_captured(Some(transaction), &captured.aggregate.id, &project)
            .await?)
    }
}
