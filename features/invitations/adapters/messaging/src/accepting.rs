use accounts_contract::{AccountEventDTO, INVITATION_TAKEN_UP};
use async_trait::async_trait;
use eventsourcing::Agent;
use inbox::{KeptHandler, Refused};
use invitations_core::{InvitationId, InvitationsCore};
use messaging::Message;
use transacting::Transaction;

pub struct AcceptTakenUpInvitationListener {
    invitations: InvitationsCore,
}

impl AcceptTakenUpInvitationListener {
    pub fn new(invitations: InvitationsCore) -> Self {
        Self { invitations }
    }
}

#[async_trait]
impl KeptHandler for AcceptTakenUpInvitationListener {
    const NAME: &'static str = "accept-taken-up-invitation";
    const LISTENS_TO: &'static [&'static str] = &[INVITATION_TAKEN_UP];
    const WHEN_REFUSED: &'static str = "An invitation an account took up still shows as open, and nobody joins the project it was into.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Refused> {
        let told = eventpublishing::published_in::<AccountEventDTO>(message)?;
        let AccountEventDTO::InvitationTakenUp { invitation } = told.event.body else {
            return Err("an invitation taken up names no invitation".into());
        };
        let invitation: InvitationId = invitation.parse()?;

        self.invitations
            .taken_up(
                Some(transaction),
                &invitation,
                message.occurred_at,
                &Agent::Account(told.aggregate.id.into()),
            )
            .await
            .map_err(Refused::of)
    }
}
