use async_trait::async_trait;
use inbox::KeptHandler;
use invitations_contract::ISSUED;
use invitations_core::{InvitationId, InvitationsCore};
use messaging::{Failure, Message};
use transacting::Transaction;

pub struct MailIssuedInvitationListener {
    invitations: InvitationsCore,
}

impl MailIssuedInvitationListener {
    pub fn new(invitations: InvitationsCore) -> Self {
        Self { invitations }
    }
}

#[async_trait]
impl KeptHandler for MailIssuedInvitationListener {
    const NAME: &'static str = "mail-issued-invitation";
    const LISTENS_TO: &'static [&'static str] = &[ISSUED];
    const WHEN_REFUSED: &'static str =
        "An invitation was issued but not mailed, so whom it is for has no link to follow.";

    async fn take_in(
        &self,
        _transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        Ok(self
            .invitations
            .mail_issued(&eventpublishing::aggregate_in::<InvitationId>(message)?)
            .await?)
    }
}
