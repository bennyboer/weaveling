use accounts_core::{AccountsCore, EmailAddress, InvitationScope, InvitationTicket, TokenDigest};
use async_trait::async_trait;
use clock::text;
use inbox::{KeptHandler, Refused};
use invitations_contract::{DECLINED, ISSUED, InvitationEventDTO, RETIRED, ScopeDTO, WITHDRAWN};
use messaging::Message;
use transacting::Transaction;

pub struct KeepInvitationTicketsListener {
    accounts: AccountsCore,
}

impl KeepInvitationTicketsListener {
    pub fn new(accounts: AccountsCore) -> Self {
        Self { accounts }
    }
}

#[async_trait]
impl KeptHandler for KeepInvitationTicketsListener {
    const NAME: &'static str = "keep-invitation-tickets";
    const LISTENS_TO: &'static [&'static str] = &[ISSUED, WITHDRAWN, DECLINED, RETIRED];
    const WHEN_REFUSED: &'static str = "An invitation did not reach accounts, so its link may not let its address sign up, or still may after being withdrawn or turned down.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Refused> {
        let told = eventpublishing::published_in::<InvitationEventDTO>(message)?;
        let invitation = told.aggregate.id;

        match told.event.body {
            InvitationEventDTO::Issued {
                to,
                digest,
                until,
                scope,
                by_an_operator,
            } => {
                let ticket = InvitationTicket {
                    invitation,
                    to: EmailAddress::new(&to)?,
                    until: text::read(&until).ok_or("an issued invitation holds until no time")?,
                    scope: match scope {
                        ScopeDTO::Instance => InvitationScope::Instance,
                        ScopeDTO::Project { project, .. } => InvitationScope::Project { project },
                    },
                    by_an_operator,
                };

                self.accounts
                    .keep_invitation_ticket(Some(transaction), &TokenDigest::kept(digest), &ticket)
                    .await
                    .map_err(Refused::of)
            }
            InvitationEventDTO::Withdrawn | InvitationEventDTO::Declined { .. } => self
                .accounts
                .forget_invitation_ticket(Some(transaction), &invitation)
                .await
                .map_err(Refused::of),
            InvitationEventDTO::Retired => self
                .accounts
                .drop_invitation_ticket(Some(transaction), &invitation)
                .await
                .map_err(Refused::of),
            InvitationEventDTO::Accepted { .. } => Ok(()),
        }
    }
}
