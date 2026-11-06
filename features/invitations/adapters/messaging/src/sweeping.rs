use async_trait::async_trait;
use invitations_core::InvitationsCore;
use messaging::{Failure, Handler, Message, WithoutInbox};
use serde::{Deserialize, Serialize};

pub const SWEEP_PROJECT: &str = "follow-up.invitation.sweep-project";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SweepProjectDTO {
    pub project: String,
}

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
    const LISTENS_TO: &'static [&'static str] = &[SWEEP_PROJECT];
    const WHEN_REFUSED: &'static str = "The open invitations into a deleted project were not withdrawn, so their links may still be taken up.";
    const WITHOUT_INBOX: WithoutInbox = WithoutInbox::Sweep;

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let asked: SweepProjectDTO = message.payload_as()?;

        Ok(self.invitations.sweep(&asked.project).await?)
    }
}
