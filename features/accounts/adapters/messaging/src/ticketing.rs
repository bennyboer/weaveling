use accounts_core::{AccountsCore, EmailAddress, InvitationScope, InvitationTicket, TokenDigest};
use async_trait::async_trait;
use clock::text;
use inbox::KeptHandler;
use invitations_contract::{DECLINED, ISSUED, InvitationEventDTO, RETIRED, ScopeDTO, WITHDRAWN};
use messaging::{Failure, Message};
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
    const WHEN_REFUSED: &'static str = "A new or changed invitation did not reach the accounts feature. Its link may not let the invited address sign up. Or the link may still work after the invitation was withdrawn or turned down.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
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

                Ok(self
                    .accounts
                    .keep_invitation_ticket(Some(transaction), &TokenDigest::kept(digest), &ticket)
                    .await?)
            }
            InvitationEventDTO::Withdrawn | InvitationEventDTO::Declined { .. } => Ok(self
                .accounts
                .forget_invitation_ticket(Some(transaction), &invitation)
                .await?),
            InvitationEventDTO::Retired => Ok(self
                .accounts
                .drop_invitation_ticket(Some(transaction), &invitation)
                .await?),
            InvitationEventDTO::Accepted { .. } => Ok(()),
        }
    }
}
