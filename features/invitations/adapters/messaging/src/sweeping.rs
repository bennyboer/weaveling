use async_trait::async_trait;
use invitations_core::{InvitationsCore, ProjectLink};
use messaging::{Failure, Handler, Message, RoutingKey};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;

pub const SWEEP_PROJECT: &str = "follow-up.invitation.sweep-project";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SweepProjectDTO {
    pub project: String,
}

pub(crate) fn sweep_project(
    heard: &Message,
    project: &ProjectLink,
    now: OffsetDateTime,
) -> Message {
    heard.answering(
        RoutingKey::parse(SWEEP_PROJECT).expect("a follow-up's routing key holds no wildcards"),
        json!(SweepProjectDTO {
            project: project.to_string(),
        }),
        now,
    )
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

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let asked: SweepProjectDTO = message.payload_as()?;

        Ok(self.invitations.sweep(&asked.project).await?)
    }
}
