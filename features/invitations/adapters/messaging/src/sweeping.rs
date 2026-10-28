use async_trait::async_trait;
use invitations_core::InvitationsCore;
use messaging::{Failure, Handler, Message};
use projects_contract::{DELETED, ProjectEventDTO};

pub struct WithdrawInvitationsOfDeletedProjectListener {
    invitations: InvitationsCore,
}

impl WithdrawInvitationsOfDeletedProjectListener {
    pub fn new(invitations: InvitationsCore) -> Self {
        Self { invitations }
    }
}

#[async_trait]
impl Handler for WithdrawInvitationsOfDeletedProjectListener {
    const NAME: &'static str = "withdraw-invitations-of-deleted-project";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str = "The open invitations into a deleted project were not withdrawn, so their links may still be taken up.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let deleted = eventpublishing::published_in::<ProjectEventDTO>(message)?;

        Ok(self.invitations.sweep(&deleted.aggregate.id).await?)
    }
}
