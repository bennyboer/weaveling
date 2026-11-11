use appearances_core::AppearancesCore;
use inbox::KeptHandler;
use messaging::{Failure, Message};
use projects_contract::context::PROJECT;
use projects_contract::{DELETED, ProjectEventDTO};
use transacting::Transaction;

pub(crate) fn project_of(message: &Message) -> Option<String> {
    eventpublishing::context_in(message)
        .get(PROJECT)
        .map(str::to_owned)
}

pub struct ForgetDeletedProjectListener {
    appearances: AppearancesCore,
}

impl ForgetDeletedProjectListener {
    pub fn new(appearances: AppearancesCore) -> Self {
        Self { appearances }
    }
}

#[async_trait::async_trait]
impl KeptHandler for ForgetDeletedProjectListener {
    const NAME: &'static str = "forget-appearances-of-deleted-project";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str = "Where ideas of a deleted project appeared is still kept.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let deleted = eventpublishing::published_in::<ProjectEventDTO>(message)?;

        Ok(self
            .appearances
            .project_deleted(
                Some(transaction),
                &deleted.aggregate.id,
                message.occurred_at,
            )
            .await?)
    }
}
