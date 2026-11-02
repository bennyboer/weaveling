use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

use accounts_core::{
    AccountId, Attempt, CodePurpose, Dismissal, EmailAddress, EmailAddresses, InvitationTicket,
    InvitationTickets, Operators, PasswordHash, Passwords, PendingVerification, SecretsError,
    Session, Sessions, TokenDigest, VerificationCode, Verifications,
};
use async_trait::async_trait;
use time::OffsetDateTime;
use transacting::{Transaction, memory};

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
    kept: Arc<Mutex<Kept>>,
}

impl InMemorySecrets {
    fn kept(&self) -> MutexGuard<'_, Kept> {
        self.kept.lock().expect("the secrets are not poisoned")
    }

    fn undo_on_rollback(
        &self,
        transaction: &mut Transaction,
        restore: impl FnOnce(&mut Kept) + Send + 'static,
    ) -> Result<(), SecretsError> {
        let kept = self.kept.clone();

        memory::undo_on_rollback(transaction, move || {
            restore(&mut kept.lock().expect("the secrets are not poisoned"));
        })
        .map_err(|failure| SecretsError::Backend(Box::new(failure)))
    }
}

#[async_trait]
impl EmailAddresses for InMemorySecrets {
    async fn claim(
        &self,
        transaction: &mut Transaction,
        email: &EmailAddress,
        account: &AccountId,
        at: OffsetDateTime,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        if kept.email_addresses.contains_key(email) {
            return Err(SecretsError::Taken);
        }
        let claimed = email.clone();
        self.undo_on_rollback(transaction, move |kept| {
            kept.email_addresses.remove(&claimed);
        })?;
        kept.email_addresses.insert(email.clone(), (*account, at));

        Ok(())
    }

    async fn take_over(
        &self,
        transaction: &mut Transaction,
        email: &EmailAddress,
        from: &AccountId,
        to: &AccountId,
        claimed_before: OffsetDateTime,
        at: OffsetDateTime,
    ) -> Result<bool, SecretsError> {
        let mut kept = self.kept();
        match kept.email_addresses.get(email).copied() {
            Some(claim) if claim.0 == *from && claim.1 < claimed_before => {
                let claimed = email.clone();
                self.undo_on_rollback(transaction, move |kept| {
                    kept.email_addresses.insert(claimed, claim);
                })?;
                kept.email_addresses.insert(email.clone(), (*to, at));
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

    async fn release(
        &self,
        transaction: &mut Transaction,
        email: &EmailAddress,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        let Some(claim) = kept.email_addresses.get(email).copied() else {
            return Ok(());
        };
        if claim.0 != *account {
            return Ok(());
        }

        let claimed = email.clone();
        self.undo_on_rollback(transaction, move |kept| {
            kept.email_addresses.insert(claimed, claim);
        })?;
        kept.email_addresses.remove(email);

        Ok(())
    }
}

#[async_trait]
impl Passwords for InMemorySecrets {
    async fn keep(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        password: &PasswordHash,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        let before = kept.passwords.get(account).cloned();

        let holder = *account;
        self.undo_on_rollback(transaction, move |kept| match before {
            Some(before) => {
                kept.passwords.insert(holder, before);
            }
            None => {
                kept.passwords.remove(&holder);
            }
        })?;
        kept.passwords.insert(*account, password.clone());

        Ok(())
    }

    async fn of(&self, account: &AccountId) -> Result<Option<PasswordHash>, SecretsError> {
        Ok(self.kept().passwords.get(account).cloned())
    }
}

#[async_trait]
impl Sessions for InMemorySecrets {
    async fn open(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
        session: &Session,
    ) -> Result<(), SecretsError> {
        let replaced = self.kept().sessions.insert(digest.clone(), session.clone());
        let opened = digest.clone();
        self.undo_on_rollback(transaction, move |kept| match replaced {
            Some(before) => {
                kept.sessions.insert(opened, before);
            }
            None => {
                kept.sessions.remove(&opened);
            }
        })
    }

    async fn find(&self, digest: &TokenDigest) -> Result<Option<Session>, SecretsError> {
        Ok(self.kept().sessions.get(digest).cloned())
    }

    async fn end(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<(), SecretsError> {
        let ended = self.kept().sessions.remove(digest);
        let digest = digest.clone();
        self.undo_on_rollback(transaction, move |kept| {
            if let Some(session) = ended {
                kept.sessions.insert(digest, session);
            }
        })
    }
}

#[async_trait]
impl Verifications for InMemorySecrets {
    async fn issue(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        purpose: CodePurpose,
        pending: &PendingVerification,
    ) -> Result<(), SecretsError> {
        let replaced = self
            .kept()
            .pending
            .insert((*account, purpose), pending.clone());
        let issued = (*account, purpose);
        self.undo_on_rollback(transaction, move |kept| match replaced {
            Some(before) => {
                kept.pending.insert(issued, before);
            }
            None => {
                kept.pending.remove(&issued);
            }
        })
    }

    async fn attempt(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        purpose: CodePurpose,
        within: u32,
    ) -> Result<Attempt, SecretsError> {
        let attempt = match self.kept().pending.get_mut(&(*account, purpose)) {
            None => Attempt::Nothing,
            Some(pending) if pending.tried >= within => Attempt::Exhausted,
            Some(pending) => {
                pending.tried += 1;
                Attempt::Counted(pending.clone())
            }
        };
        if let Attempt::Counted(_) = attempt {
            let attempted = (*account, purpose);
            self.undo_on_rollback(transaction, move |kept| {
                if let Some(pending) = kept.pending.get_mut(&attempted) {
                    pending.tried -= 1;
                }
            })?;
        }

        Ok(attempt)
    }

    async fn close(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        purpose: CodePurpose,
        code: &VerificationCode,
    ) -> Result<bool, SecretsError> {
        let key = (*account, purpose);
        let closed = {
            let mut kept = self.kept();
            match kept.pending.get(&key) {
                Some(pending) if pending.code.as_str() == code.as_str() => {
                    kept.pending.remove(&key)
                }
                _ => None,
            }
        };
        let taken = closed.is_some();
        self.undo_on_rollback(transaction, move |kept| {
            if let Some(pending) = closed {
                kept.pending.insert(key, pending);
            }
        })?;

        Ok(taken)
    }
}

#[async_trait]
impl Operators for InMemorySecrets {
    async fn keep_code(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<(), SecretsError> {
        let replaced = self.kept().operator_code.replace(digest.clone());
        self.undo_on_rollback(transaction, move |kept| {
            kept.operator_code = replaced;
        })
    }

    async fn holds_code(&self, digest: &TokenDigest) -> Result<bool, SecretsError> {
        Ok(self.kept().operator_code.as_ref() == Some(digest))
    }

    async fn take_code(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<bool, SecretsError> {
        let mut kept = self.kept();
        if kept.operator_code.as_ref() != Some(digest) {
            return Ok(false);
        }
        kept.operator_code = None;
        let taken = digest.clone();
        self.undo_on_rollback(transaction, move |kept| {
            kept.operator_code = Some(taken);
        })?;

        Ok(true)
    }

    async fn appoint(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        let appointed = self.kept().operators.insert(*account);
        let account = *account;
        self.undo_on_rollback(transaction, move |kept| {
            if appointed {
                kept.operators.remove(&account);
            }
        })
    }

    async fn dismiss(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<Dismissal, SecretsError> {
        let mut kept = self.kept();
        if !kept.operators.contains(account) {
            return Ok(Dismissal::NotAppointed);
        }
        if kept.operators.len() == 1 {
            return Ok(Dismissal::LastOne);
        }
        kept.operators.remove(account);
        let dismissed = *account;
        self.undo_on_rollback(transaction, move |kept| {
            kept.operators.insert(dismissed);
        })?;

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
        transaction: &mut Transaction,
        digest: &TokenDigest,
        ticket: &InvitationTicket,
    ) -> Result<bool, SecretsError> {
        let mut kept = self.kept();
        let digest_held = kept
            .invitation_tickets
            .values()
            .any(|(held, _)| held == digest);
        let fresh = !digest_held
            && !kept.invitation_tickets.contains_key(&ticket.invitation)
            && !kept.ended_invitations.contains(&ticket.invitation);
        if fresh {
            let invitation = ticket.invitation.clone();
            self.undo_on_rollback(transaction, move |kept| {
                kept.invitation_tickets.remove(&invitation);
            })?;
            kept.invitation_tickets
                .insert(ticket.invitation.clone(), (digest.clone(), ticket.clone()));
        }

        Ok(fresh)
    }

    async fn find_ticket(
        &self,
        digest: &TokenDigest,
    ) -> Result<Option<InvitationTicket>, SecretsError> {
        let kept = self.kept();

        Ok(kept
            .invitation_tickets
            .iter()
            .find(|(invitation, (held, _))| {
                held == digest && !kept.ended_invitations.contains(*invitation)
            })
            .map(|(_, (_, ticket))| ticket.clone()))
    }

    async fn take_ticket(
        &self,
        transaction: &mut Transaction,
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
            let invitation = ticket.invitation.clone();
            self.undo_on_rollback(transaction, move |kept| {
                kept.ended_invitations.remove(&invitation);
            })?;
            kept.ended_invitations.insert(ticket.invitation.clone());
        }

        Ok(open)
    }

    async fn digest_of_ticket(
        &self,
        invitation: &str,
    ) -> Result<Option<TokenDigest>, SecretsError> {
        let kept = self.kept();
        if kept.ended_invitations.contains(invitation) {
            return Ok(None);
        }

        Ok(kept
            .invitation_tickets
            .get(invitation)
            .map(|(digest, _)| digest.clone()))
    }

    async fn forget_ticket(
        &self,
        transaction: &mut Transaction,
        invitation: &str,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        if kept.ended_invitations.contains(invitation) {
            return Ok(());
        }

        let ended = invitation.to_owned();
        self.undo_on_rollback(transaction, move |kept| {
            kept.ended_invitations.remove(&ended);
        })?;
        kept.ended_invitations.insert(invitation.to_owned());

        Ok(())
    }

    async fn drop_ticket(
        &self,
        transaction: &mut Transaction,
        invitation: &str,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        let held = kept.invitation_tickets.get(invitation).cloned();
        let ended = kept.ended_invitations.contains(invitation);

        let dropped = invitation.to_owned();
        self.undo_on_rollback(transaction, move |kept| {
            if let Some(held) = held {
                kept.invitation_tickets.insert(dropped.clone(), held);
            }
            if ended {
                kept.ended_invitations.insert(dropped);
            }
        })?;
        kept.invitation_tickets.remove(invitation);
        kept.ended_invitations.remove(invitation);

        Ok(())
    }
}
