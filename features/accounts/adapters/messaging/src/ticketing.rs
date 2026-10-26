use accounts_core::{AccountsCore, EmailAddress, InvitationTicket, TokenDigest};
use async_trait::async_trait;
use clock::text;
use invitations_contract::{ISSUED, InvitationEventDTO, RETIRED, ScopeDTO, WITHDRAWN};
use messaging::{Failure, Handler, Message};

pub struct KeepInvitationTicketsListener {
    accounts: AccountsCore,
}

impl KeepInvitationTicketsListener {
    pub fn new(accounts: AccountsCore) -> Self {
        Self { accounts }
    }
}

#[async_trait]
impl Handler for KeepInvitationTicketsListener {
    const NAME: &'static str = "keep-invitation-tickets";
    const LISTENS_TO: &'static [&'static str] = &[ISSUED, WITHDRAWN, RETIRED];
    const WHEN_REFUSED: &'static str = "An invitation did not reach accounts, so its link may not let its address sign up, or still may after being withdrawn.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<InvitationEventDTO>(message)?;
        let invitation = told.aggregate.id;

        match told.event.body {
            // TODO 5b-2: project invitations keep no ticket yet, so an account is never declined for one; accounts will keep their tickets with the scope, decline for the instance alone, and let an account take a project's up
            InvitationEventDTO::Issued {
                scope: ScopeDTO::Project { .. },
                ..
            } => Ok(()),
            InvitationEventDTO::Issued {
                to, digest, until, ..
            } => {
                let ticket = InvitationTicket {
                    invitation,
                    to: EmailAddress::new(&to)?,
                    until: text::read(&until).ok_or("an issued invitation holds until no time")?,
                };

                Ok(self
                    .accounts
                    .keep_invitation_ticket(&TokenDigest::kept(digest), &ticket)
                    .await?)
            }
            InvitationEventDTO::Withdrawn => {
                Ok(self.accounts.forget_invitation_ticket(&invitation).await?)
            }
            InvitationEventDTO::Retired => {
                Ok(self.accounts.drop_invitation_ticket(&invitation).await?)
            }
            InvitationEventDTO::Accepted | InvitationEventDTO::Declined { .. } => Ok(()),
        }
    }
}
