use async_trait::async_trait;
use invitations_contract::EVERY_INVITATION;
use invitations_core::{InvitationId, InvitationsCore};
use messaging::{Failure, Handler, Message};

pub struct CatalogueInvitationsListener {
    invitations: InvitationsCore,
}

impl CatalogueInvitationsListener {
    pub fn new(invitations: InvitationsCore) -> Self {
        Self { invitations }
    }
}

#[async_trait]
impl Handler for CatalogueInvitationsListener {
    const NAME: &'static str = "catalogue-invitation";
    const LISTENS_TO: &'static [&'static str] = &[EVERY_INVITATION];
    const WHEN_REFUSED: &'static str =
        "A change to an invitation did not reach the operators' list.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        Ok(self
            .invitations
            .catalogue(&eventpublishing::aggregate_in::<InvitationId>(message)?)
            .await?)
    }
}
