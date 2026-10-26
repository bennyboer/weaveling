use accounts_contract::{AccountEventDTO, INVITATION_DECLINED, decline_reason};
use async_trait::async_trait;
use invitations_core::{DeclineReason, InvitationId, InvitationsCore};
use messaging::{Failure, Handler, Message};

pub struct DeclineInvitationListener {
    invitations: InvitationsCore,
}

impl DeclineInvitationListener {
    pub fn new(invitations: InvitationsCore) -> Self {
        Self { invitations }
    }
}

#[async_trait]
impl Handler for DeclineInvitationListener {
    const NAME: &'static str = "decline-invitation";
    const LISTENS_TO: &'static [&'static str] = &[INVITATION_DECLINED];
    const WHEN_REFUSED: &'static str = "An invitation to an address that has an account already still shows as open to the operators.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<AccountEventDTO>(message)?;
        let AccountEventDTO::InvitationDeclined { invitation, reason } = told.event.body else {
            return Err("a declined invitation names no invitation".into());
        };
        let invitation: InvitationId = invitation.parse()?;
        let reason = match reason.as_str() {
            decline_reason::ADDRESS_HAS_ACCOUNT => DeclineReason::AddressHasAccount,
            _ => return Err(format!("{reason} is no reason for declining").into()),
        };

        Ok(self.invitations.declined(&invitation, reason).await?)
    }
}
