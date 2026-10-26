use accounts_contract::{AccountEventDTO, SIGNED_UP_BY_INVITATION};
use async_trait::async_trait;
use eventsourcing::Agent;
use invitations_core::{InvitationId, InvitationsCore};
use messaging::{Failure, Handler, Message};

pub struct AcceptTakenUpInvitationListener {
    invitations: InvitationsCore,
}

impl AcceptTakenUpInvitationListener {
    pub fn new(invitations: InvitationsCore) -> Self {
        Self { invitations }
    }
}

#[async_trait]
impl Handler for AcceptTakenUpInvitationListener {
    const NAME: &'static str = "accept-taken-up-invitation";
    const LISTENS_TO: &'static [&'static str] = &[SIGNED_UP_BY_INVITATION];
    const WHEN_REFUSED: &'static str =
        "An invitation an account was made by still shows as open to the operators.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<AccountEventDTO>(message)?;
        let AccountEventDTO::SignedUpByInvitation { invitation } = told.event.body else {
            return Err("a sign-up by invitation names no invitation".into());
        };
        let invitation: InvitationId = invitation.parse()?;

        Ok(self
            .invitations
            .taken_up(
                &invitation,
                message.occurred_at,
                &Agent::Account(told.aggregate.id.into()),
            )
            .await?)
    }
}
