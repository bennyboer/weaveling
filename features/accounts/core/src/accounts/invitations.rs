use eventsourcing::{Agent, AggregateId, Version};
use transacting::Transaction;

use super::AccountsCore;
use crate::account::AccountCommand;
use crate::decline::DeclineReason;
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::session::TokenDigest;
use crate::ticket::{InvitationScope, InvitationTicket, InvitationToken};

impl AccountsCore {
    pub async fn take_up_invitation(
        &self,
        id: &AccountId,
        token: &str,
    ) -> Result<InvitationTicket, AccountsError> {
        let digest = InvitationToken::presented(token).digest();
        let found = self.found_ticket(id, &digest).await?;

        self.taken_up(id, &digest, &found).await
    }

    pub async fn take_up_invitation_by_id(
        &self,
        id: &AccountId,
        invitation: &str,
    ) -> Result<InvitationTicket, AccountsError> {
        let (digest, found) = self.found_ticket_by_id(id, invitation).await?;

        self.taken_up(id, &digest, &found).await
    }

    pub async fn decline_invitation(
        &self,
        id: &AccountId,
        token: &str,
    ) -> Result<(), AccountsError> {
        let digest = InvitationToken::presented(token).digest();
        self.found_ticket(id, &digest).await?;

        self.turned_down(id, &digest).await
    }

    pub async fn decline_invitation_by_id(
        &self,
        id: &AccountId,
        invitation: &str,
    ) -> Result<(), AccountsError> {
        let (digest, _found) = self.found_ticket_by_id(id, invitation).await?;

        self.turned_down(id, &digest).await
    }

    async fn found_ticket_by_id(
        &self,
        id: &AccountId,
        invitation: &str,
    ) -> Result<(TokenDigest, InvitationTicket), AccountsError> {
        let Some(digest) = self.invitation_tickets.digest_of_ticket(invitation).await? else {
            return Err(AccountsError::InvitationInvalid);
        };

        match self.found_ticket(id, &digest).await {
            Ok(found) => Ok((digest, found)),
            Err(AccountsError::InvitationForAnotherAddress { .. }) => {
                Err(AccountsError::InvitationInvalid)
            }
            Err(refused) => Err(refused),
        }
    }

    async fn found_ticket(
        &self,
        id: &AccountId,
        digest: &TokenDigest,
    ) -> Result<InvitationTicket, AccountsError> {
        let Some(found) = self.invitation_tickets.find_ticket(digest).await? else {
            return Err(AccountsError::InvitationInvalid);
        };
        if found.to != *self.get(id).await?.state.email() {
            return Err(AccountsError::InvitationForAnotherAddress { to: found.to });
        }
        if self.clock.now() > found.until {
            return Err(AccountsError::InvitationExpired);
        }

        Ok(found)
    }

    async fn claimed_ticket(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<InvitationTicket, AccountsError> {
        self.invitation_tickets
            .take_ticket(transaction, digest)
            .await?
            .ok_or(AccountsError::InvitationInvalid)
    }

    async fn taken_up(
        &self,
        id: &AccountId,
        digest: &TokenDigest,
        found: &InvitationTicket,
    ) -> Result<InvitationTicket, AccountsError> {
        if found.scope == InvitationScope::Instance {
            return Err(AccountsError::InvitationInvalid);
        }
        transacting::retrying!(&*self.transactions, |transaction| {
            self.not_paused(&found.scope).await?;
            let ticket = self.claimed_ticket(transaction, digest).await?;
            self.events
                .execute(
                    transaction,
                    &AggregateId::from(id),
                    AccountCommand::TakeUpInvitation {
                        invitation: ticket.invitation.clone(),
                    },
                    &id.acting(),
                )
                .await?;
            Ok(ticket)
        })
    }

    async fn turned_down(&self, id: &AccountId, digest: &TokenDigest) -> Result<(), AccountsError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            let ticket = self.claimed_ticket(transaction, digest).await?;
            self.events
                .execute(
                    transaction,
                    &AggregateId::from(id),
                    AccountCommand::DeclineInvitation {
                        invitation: ticket.invitation,
                        reason: DeclineReason::TurnedDown,
                    },
                    &id.acting(),
                )
                .await?;
            Ok(())
        })
    }

    pub async fn keep_invitation_ticket(
        &self,
        within: Option<&mut Transaction>,
        digest: &TokenDigest,
        ticket: &InvitationTicket,
    ) -> Result<(), AccountsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            let fresh = self
                .invitation_tickets
                .keep_ticket(transaction, digest, ticket)
                .await?;
            if fresh && ticket.scope == InvitationScope::Instance {
                self.declined_for_an_account(transaction, ticket).await?;
            }

            Ok(())
        })
    }

    async fn declined_for_an_account(
        &self,
        transaction: &mut Transaction,
        ticket: &InvitationTicket,
    ) -> Result<(), AccountsError> {
        let Some((id, _)) = self.whose(&ticket.to).await? else {
            return Ok(());
        };

        self.invitation_tickets
            .forget_ticket(transaction, &ticket.invitation)
            .await?;
        self.events
            .execute(
                transaction,
                &AggregateId::from(&id),
                AccountCommand::DeclineInvitation {
                    invitation: ticket.invitation.clone(),
                    reason: DeclineReason::AddressHasAccount,
                },
                &Agent::System,
            )
            .await?;

        Ok(())
    }

    pub(super) async fn not_paused(&self, scope: &InvitationScope) -> Result<(), AccountsError> {
        let InvitationScope::Project { project } = scope else {
            return Ok(());
        };
        if self.catalog.is_in_the_trash(project).await? {
            return Err(AccountsError::InvitationPaused);
        }

        Ok(())
    }

    pub async fn remember_trash(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: u64,
        in_the_trash: bool,
    ) -> Result<(), AccountsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .remember_trash(transaction, project, Version::of(version), in_the_trash)
                .await?;

            Ok(())
        })
    }

    pub async fn forget_trash(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
    ) -> Result<(), AccountsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog.forget_trash(transaction, project).await?;

            Ok(())
        })
    }

    pub async fn forget_invitation_ticket(
        &self,
        within: Option<&mut Transaction>,
        invitation: &str,
    ) -> Result<(), AccountsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.invitation_tickets
                .forget_ticket(transaction, invitation)
                .await?;

            Ok(())
        })
    }

    pub async fn drop_invitation_ticket(
        &self,
        within: Option<&mut Transaction>,
        invitation: &str,
    ) -> Result<(), AccountsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.invitation_tickets
                .drop_ticket(transaction, invitation)
                .await?;

            Ok(())
        })
    }
}
