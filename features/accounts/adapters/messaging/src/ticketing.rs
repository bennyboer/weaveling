use std::sync::Arc;

use accounts_core::{EmailAddress, InvitationTicket, InvitationTickets, TokenDigest};
use async_trait::async_trait;
use clock::text;
use invitations_contract::{ISSUED, InvitationEventDTO, WITHDRAWN};
use messaging::{Failure, Handler, Message};

pub struct KeepInvitationTicketsListener {
    tickets: Arc<dyn InvitationTickets>,
}

impl KeepInvitationTicketsListener {
    pub fn new(tickets: Arc<dyn InvitationTickets>) -> Self {
        Self { tickets }
    }
}

#[async_trait]
impl Handler for KeepInvitationTicketsListener {
    const NAME: &'static str = "keep-invitation-tickets";
    const LISTENS_TO: &'static [&'static str] = &[ISSUED, WITHDRAWN];
    const WHEN_REFUSED: &'static str = "An invitation did not reach accounts, so its link may not let its address sign up, or still may after being withdrawn.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<InvitationEventDTO>(message)?;
        let invitation = told.aggregate.id;

        match told.event.body {
            InvitationEventDTO::Issued { to, digest, until } => {
                let ticket = InvitationTicket {
                    invitation,
                    to: EmailAddress::new(&to)?,
                    until: text::read(&until).ok_or("an issued invitation holds until no time")?,
                };

                Ok(self
                    .tickets
                    .keep_ticket(&TokenDigest::kept(digest), &ticket)
                    .await?)
            }
            InvitationEventDTO::Withdrawn => Ok(self.tickets.forget_ticket(&invitation).await?),
            InvitationEventDTO::Accepted => Ok(()),
        }
    }
}
