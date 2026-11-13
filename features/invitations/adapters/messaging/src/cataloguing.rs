use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use inbox::KeptHandler;
use invitations_contract::EVERY_INVITATION;
use invitations_core::{InvitationId, InvitationsCore, ProjectLink};
use messaging::{Failure, Message};
use outbox::{Origin, Outbox};
use serde::{Deserialize, Serialize};
use transacting::Transaction;

pub const WITHDRAW_IF_PROJECT_GONE: &str = "follow-up.invitation.withdraw-if-project-gone";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithdrawIfProjectGoneDTO {
    pub invitation: String,
    pub project: String,
}

pub struct CatalogueInvitationsListener {
    invitations: InvitationsCore,
    outbox: Arc<dyn Outbox>,
    clock: Arc<dyn Clock>,
}

impl CatalogueInvitationsListener {
    pub fn new(
        invitations: InvitationsCore,
        outbox: Arc<dyn Outbox>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            invitations,
            outbox,
            clock,
        }
    }
}

#[async_trait]
impl KeptHandler for CatalogueInvitationsListener {
    const NAME: &'static str = "catalogue-invitation";
    const LISTENS_TO: &'static [&'static str] = &[EVERY_INVITATION];
    const WHEN_REFUSED: &'static str =
        "A change to an invitation did not reach the operators' list of invitations.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let invitation = eventpublishing::aggregate_in::<InvitationId>(message)?;
        let Some(project) = self
            .invitations
            .catalogue(Some(&mut *transaction), &invitation)
            .await?
        else {
            return Ok(());
        };

        self.outbox
            .enqueue_follow_up_within(
                transaction,
                Origin::unversioned(&invitation.to_string(), "invitation"),
                message,
                WITHDRAW_IF_PROJECT_GONE,
                &WithdrawIfProjectGoneDTO {
                    invitation: invitation.to_string(),
                    project: project.to_string(),
                },
                self.clock.now(),
            )
            .await?;

        Ok(())
    }
}

pub struct WithdrawIfProjectGoneListener {
    invitations: InvitationsCore,
}

impl WithdrawIfProjectGoneListener {
    pub fn new(invitations: InvitationsCore) -> Self {
        Self { invitations }
    }
}

#[async_trait]
impl KeptHandler for WithdrawIfProjectGoneListener {
    const NAME: &'static str = "withdraw-invitation-if-project-gone";
    const LISTENS_TO: &'static [&'static str] = &[WITHDRAW_IF_PROJECT_GONE];
    const WHEN_REFUSED: &'static str = "An invitation was not withdrawn, although its project was deleted in the meantime. Its link may still be used to accept it.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let asked: WithdrawIfProjectGoneDTO = message.payload_as()?;
        let invitation: InvitationId = asked.invitation.parse()?;

        Ok(self
            .invitations
            .withdraw_if_project_gone(
                Some(transaction),
                &invitation,
                &ProjectLink::from(asked.project),
            )
            .await?)
    }
}
