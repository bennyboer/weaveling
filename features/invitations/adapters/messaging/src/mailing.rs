use async_trait::async_trait;
use inbox::EffectHandler;
use invitations_contract::ISSUED;
use invitations_core::{InvitationId, InvitationsCore};
use messaging::{Failure, Message};

pub struct MailIssuedInvitationListener {
    invitations: InvitationsCore,
}

impl MailIssuedInvitationListener {
    pub fn new(invitations: InvitationsCore) -> Self {
        Self { invitations }
    }
}

#[async_trait]
impl EffectHandler for MailIssuedInvitationListener {
    const NAME: &'static str = "mail-issued-invitation";
    const LISTENS_TO: &'static [&'static str] = &[ISSUED];
    const WHEN_REFUSED: &'static str = "An invitation was created but not sent by email. The invited person has no link to follow.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        Ok(self
            .invitations
            .mail_issued(&eventpublishing::aggregate_in::<InvitationId>(message)?)
            .await?)
    }
}
