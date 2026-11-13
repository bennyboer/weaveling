use accounts_core::AccountsCore;
use async_trait::async_trait;
use inbox::KeptHandler;
use messaging::{Failure, Message};
use projects_contract::{DELETED, ProjectEventDTO, RESTORED, TRASHED};
use transacting::Transaction;

pub struct RememberTrashedProjectsListener {
    accounts: AccountsCore,
}

impl RememberTrashedProjectsListener {
    pub fn new(accounts: AccountsCore) -> Self {
        Self { accounts }
    }
}

#[async_trait]
impl KeptHandler for RememberTrashedProjectsListener {
    const NAME: &'static str = "accounts-remember-trashed-projects";
    const LISTENS_TO: &'static [&'static str] = &[TRASHED, RESTORED, DELETED];
    const WHEN_REFUSED: &'static str = "The accounts feature did not learn that a project was put in the trash, restored or deleted. Someone may accept an invitation into the project while it is in the trash. Or an invitation may not work after the project was restored.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<ProjectEventDTO>(message)?;
        let project = told.aggregate.id;
        let in_the_trash = match told.event.body {
            ProjectEventDTO::Trashed { .. } => true,
            ProjectEventDTO::Restored => false,
            ProjectEventDTO::Deleted => {
                return Ok(self
                    .accounts
                    .forget_trash(Some(transaction), &project)
                    .await?);
            }
            _ => return Err("this tells neither a trashing, a restoring nor a deletion".into()),
        };

        Ok(self
            .accounts
            .remember_trash(
                Some(transaction),
                &project,
                told.aggregate.version,
                in_the_trash,
            )
            .await?)
    }
}
