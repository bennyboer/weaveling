use async_trait::async_trait;
use inbox::KeptHandler;
use members_contract::{ABANDONED, MembersEventDTO};
use messaging::{Failure, Message};
use projects_core::ProjectsCore;
use transacting::Transaction;

pub struct DeleteAbandonedProjectListener {
    projects: ProjectsCore,
}

impl DeleteAbandonedProjectListener {
    pub fn new(projects: ProjectsCore) -> Self {
        Self { projects }
    }
}

#[async_trait]
impl KeptHandler for DeleteAbandonedProjectListener {
    const NAME: &'static str = "delete-abandoned-project";
    const LISTENS_TO: &'static [&'static str] = &[ABANDONED];
    const WHEN_REFUSED: &'static str = "A project nobody is left in, its last member's account deleted, was not deleted, so it and its content are kept for nobody.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<MembersEventDTO>(message)?;
        let MembersEventDTO::Abandoned = told.event.body else {
            return Err("this tells no project abandoned".into());
        };

        Ok(self
            .projects
            .delete_abandoned(Some(transaction), &told.aggregate.id)
            .await?)
    }
}
