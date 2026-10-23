use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard};

use accounts_core::{
    AccountId, Attempt, CodePurpose, Dismissal, EmailAddress, EmailAddresses, InvitationTicket,
    InvitationTickets, Operators, PasswordHash, Passwords, PendingVerification, SecretsError,
    Session, Sessions, TokenDigest, Verifications,
};
use async_trait::async_trait;
use time::OffsetDateTime;

#[derive(Default)]
struct Kept {
    email_addresses: HashMap<EmailAddress, (AccountId, OffsetDateTime)>,
    passwords: HashMap<AccountId, PasswordHash>,
    pending: HashMap<(AccountId, CodePurpose), PendingVerification>,
    sessions: HashMap<TokenDigest, Session>,
    operator_code: Option<TokenDigest>,
    operators: HashSet<AccountId>,
    invitation_tickets: HashMap<String, (TokenDigest, InvitationTicket)>,
    ended_invitations: HashSet<String>,
}

#[derive(Default)]
pub struct InMemorySecrets {
    kept: Mutex<Kept>,
}

impl InMemorySecrets {
    fn kept(&self) -> MutexGuard<'_, Kept> {
        self.kept.lock().expect("the secrets are not poisoned")
    }
}

#[async_trait]
impl EmailAddresses for InMemorySecrets {
    async fn claim(
        &self,
        email: &EmailAddress,
        account: &AccountId,
        at: OffsetDateTime,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        if kept.email_addresses.contains_key(email) {
            return Err(SecretsError::Taken);
        }
        kept.email_addresses.insert(email.clone(), (*account, at));

        Ok(())
    }

    async fn take_over(
        &self,
        email: &EmailAddress,
        from: &AccountId,
        to: &AccountId,
        claimed_before: OffsetDateTime,
        at: OffsetDateTime,
    ) -> Result<bool, SecretsError> {
        let mut kept = self.kept();
        match kept.email_addresses.get_mut(email) {
            Some(claim) if claim.0 == *from && claim.1 < claimed_before => {
                *claim = (*to, at);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn holder(&self, email: &EmailAddress) -> Result<Option<AccountId>, SecretsError> {
        Ok(self
            .kept()
            .email_addresses
            .get(email)
            .map(|(account, _)| *account))
    }

    async fn release(&self, email: &EmailAddress, account: &AccountId) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        if kept.email_addresses.get(email).map(|(held, _)| held) == Some(account) {
            kept.email_addresses.remove(email);
        }

        Ok(())
    }
}

#[async_trait]
impl Passwords for InMemorySecrets {
    async fn keep(&self, account: &AccountId, password: &PasswordHash) -> Result<(), SecretsError> {
        self.kept().passwords.insert(*account, password.clone());

        Ok(())
    }

    async fn of(&self, account: &AccountId) -> Result<Option<PasswordHash>, SecretsError> {
        Ok(self.kept().passwords.get(account).cloned())
    }
}

#[async_trait]
impl Sessions for InMemorySecrets {
    async fn open(&self, digest: &TokenDigest, session: &Session) -> Result<(), SecretsError> {
        self.kept().sessions.insert(digest.clone(), session.clone());

        Ok(())
    }

    async fn find(&self, digest: &TokenDigest) -> Result<Option<Session>, SecretsError> {
        Ok(self.kept().sessions.get(digest).cloned())
    }

    async fn end(&self, digest: &TokenDigest) -> Result<(), SecretsError> {
        self.kept().sessions.remove(digest);

        Ok(())
    }
}

#[async_trait]
impl Verifications for InMemorySecrets {
    async fn issue(
        &self,
        account: &AccountId,
        purpose: CodePurpose,
        pending: &PendingVerification,
    ) -> Result<(), SecretsError> {
        self.kept()
            .pending
            .insert((*account, purpose), pending.clone());

        Ok(())
    }

    async fn attempt(
        &self,
        account: &AccountId,
        purpose: CodePurpose,
        within: u32,
    ) -> Result<Attempt, SecretsError> {
        Ok(match self.kept().pending.get_mut(&(*account, purpose)) {
            None => Attempt::Nothing,
            Some(pending) if pending.tried >= within => Attempt::Exhausted,
            Some(pending) => {
                pending.tried += 1;
                Attempt::Counted(pending.clone())
            }
        })
    }

    async fn close(&self, account: &AccountId, purpose: CodePurpose) -> Result<(), SecretsError> {
        self.kept().pending.remove(&(*account, purpose));

        Ok(())
    }
}

#[async_trait]
impl Operators for InMemorySecrets {
    async fn keep_code(&self, digest: &TokenDigest) -> Result<(), SecretsError> {
        self.kept().operator_code = Some(digest.clone());

        Ok(())
    }

    async fn take_code(&self, digest: &TokenDigest) -> Result<bool, SecretsError> {
        let mut kept = self.kept();
        if kept.operator_code.as_ref() != Some(digest) {
            return Ok(false);
        }
        kept.operator_code = None;

        Ok(true)
    }

    async fn appoint(&self, account: &AccountId) -> Result<(), SecretsError> {
        self.kept().operators.insert(*account);

        Ok(())
    }

    async fn dismiss(&self, account: &AccountId) -> Result<Dismissal, SecretsError> {
        let mut kept = self.kept();
        if !kept.operators.contains(account) {
            return Ok(Dismissal::NotAppointed);
        }
        if kept.operators.len() == 1 {
            return Ok(Dismissal::LastOne);
        }
        kept.operators.remove(account);

        Ok(Dismissal::Dismissed)
    }

    async fn any_appointed(&self) -> Result<bool, SecretsError> {
        Ok(!self.kept().operators.is_empty())
    }
}

#[async_trait]
impl InvitationTickets for InMemorySecrets {
    async fn keep_ticket(
        &self,
        digest: &TokenDigest,
        ticket: &InvitationTicket,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        let digest_held = kept
            .invitation_tickets
            .values()
            .any(|(held, _)| held == digest);
        if !digest_held {
            kept.invitation_tickets
                .entry(ticket.invitation.clone())
                .or_insert_with(|| (digest.clone(), ticket.clone()));
        }

        Ok(())
    }

    async fn take_ticket(
        &self,
        digest: &TokenDigest,
    ) -> Result<Option<InvitationTicket>, SecretsError> {
        let mut guard = self.kept();
        let kept = &mut *guard;
        let open = kept
            .invitation_tickets
            .iter()
            .find(|(invitation, (held, _))| {
                held == digest && !kept.ended_invitations.contains(*invitation)
            })
            .map(|(_, (_, ticket))| ticket.clone());
        if let Some(ticket) = &open {
            kept.ended_invitations.insert(ticket.invitation.clone());
        }

        Ok(open)
    }

    async fn forget_ticket(&self, invitation: &str) -> Result<(), SecretsError> {
        self.kept().ended_invitations.insert(invitation.to_owned());

        Ok(())
    }
}
