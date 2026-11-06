use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::{Arc, Mutex, MutexGuard};

use accounts_core::{
    AccountId, Attempt, CodePurpose, Dismissal, EmailAddress, EmailAddresses, InvitationTicket,
    InvitationTickets, KeptSecondFactor, Operators, PasswordHash, Passwords, PendingVerification,
    RecoveryCodeHash, SealedTotpSecret, SecondFactors, SecretsError, Session, Sessions,
    TokenDigest, TotpStep, VerificationCode, Verifications,
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
    trashed_projects: HashMap<String, (u64, bool)>,
    second_factors: HashMap<AccountId, KeptFactor>,
    recovery_codes: HashMap<AccountId, Vec<RecoveryCodeHash>>,
}

#[derive(Clone, Default)]
struct KeptFactor {
    in_use: Option<KeptSecondFactor>,
    being_set_up: Option<SealedTotpSecret>,
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

    fn undo_second_factor_on_rollback(
        &self,
        transaction: &mut Transaction,
        kept: &Kept,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        let holder = *account;
        let factor = kept.second_factors.get(account).cloned();
        let codes = kept.recovery_codes.get(account).cloned();

        self.undo_on_rollback(transaction, move |kept| {
            restore(&mut kept.second_factors, holder, factor);
            restore(&mut kept.recovery_codes, holder, codes);
        })
    }
}

fn restore<K: Eq + Hash, V>(map: &mut HashMap<K, V>, key: K, before: Option<V>) {
    match before {
        Some(before) => {
            map.insert(key, before);
        }
        None => {
            map.remove(&key);
        }
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
    ) -> Result<bool, SecretsError> {
        let mut kept = self.kept();
        if !kept.operators.is_empty() {
            return Ok(false);
        }
        let replaced = kept.operator_code.replace(digest.clone());
        drop(kept);
        self.undo_on_rollback(transaction, move |kept| {
            kept.operator_code = replaced;
        })?;

        Ok(true)
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
        if kept.operator_code.as_ref() != Some(digest) || !kept.operators.is_empty() {
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

    async fn is_appointed(
        &self,
        _transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<bool, SecretsError> {
        Ok(self.kept().operators.contains(account))
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

    async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: u64,
        in_the_trash: bool,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        let before = kept.trashed_projects.get(project).copied();
        if before.is_some_and(|(known, _)| known >= version) {
            return Ok(());
        }
        let restored = project.to_owned();
        self.undo_on_rollback(transaction, move |kept| match before {
            Some(before) => {
                kept.trashed_projects.insert(restored, before);
            }
            None => {
                kept.trashed_projects.remove(&restored);
            }
        })?;
        kept.trashed_projects
            .insert(project.to_owned(), (version, in_the_trash));

        Ok(())
    }

    async fn forget_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        let Some(before) = kept.trashed_projects.remove(project) else {
            return Ok(());
        };
        let restored = project.to_owned();
        self.undo_on_rollback(transaction, move |kept| {
            kept.trashed_projects.insert(restored, before);
        })?;

        Ok(())
    }

    async fn is_in_the_trash(
        &self,
        _transaction: &mut Transaction,
        project: &str,
    ) -> Result<bool, SecretsError> {
        Ok(self
            .kept()
            .trashed_projects
            .get(project)
            .is_some_and(|(_, in_the_trash)| *in_the_trash))
    }
}

#[async_trait]
impl SecondFactors for InMemorySecrets {
    async fn begin_setting_up(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        secret: &SealedTotpSecret,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        self.undo_second_factor_on_rollback(transaction, &kept, account)?;
        kept.second_factors
            .entry(*account)
            .or_default()
            .being_set_up = Some(secret.clone());

        Ok(())
    }

    async fn second_factor_being_set_up(
        &self,
        account: &AccountId,
    ) -> Result<Option<SealedTotpSecret>, SecretsError> {
        Ok(self
            .kept()
            .second_factors
            .get(account)
            .and_then(|factor| factor.being_set_up.clone()))
    }

    async fn finish_setting_up(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        being_set_up: &SealedTotpSecret,
        accepted: TotpStep,
    ) -> Result<bool, SecretsError> {
        let mut kept = self.kept();
        let still_being_set_up = kept
            .second_factors
            .get(account)
            .is_some_and(|factor| factor.being_set_up.as_ref() == Some(being_set_up));
        if !still_being_set_up {
            return Ok(false);
        }
        self.undo_second_factor_on_rollback(transaction, &kept, account)?;
        kept.second_factors.insert(
            *account,
            KeptFactor {
                in_use: Some(KeptSecondFactor {
                    secret: being_set_up.clone(),
                    last_accepted: accepted,
                }),
                being_set_up: None,
            },
        );

        Ok(true)
    }

    async fn second_factor_of(
        &self,
        account: &AccountId,
    ) -> Result<Option<KeptSecondFactor>, SecretsError> {
        Ok(self
            .kept()
            .second_factors
            .get(account)
            .and_then(|factor| factor.in_use.clone()))
    }

    async fn accept_step(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        step: TotpStep,
    ) -> Result<bool, SecretsError> {
        let mut kept = self.kept();
        let later = kept
            .second_factors
            .get(account)
            .and_then(|factor| factor.in_use.as_ref())
            .is_some_and(|in_use| step > in_use.last_accepted);
        if !later {
            return Ok(false);
        }
        self.undo_second_factor_on_rollback(transaction, &kept, account)?;
        if let Some(in_use) = kept
            .second_factors
            .get_mut(account)
            .and_then(|factor| factor.in_use.as_mut())
        {
            in_use.last_accepted = step;
        }

        Ok(true)
    }

    async fn keep_recovery_codes(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        codes: &[RecoveryCodeHash],
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        self.undo_second_factor_on_rollback(transaction, &kept, account)?;
        kept.recovery_codes.insert(*account, codes.to_vec());

        Ok(())
    }

    async fn recovery_codes_of(
        &self,
        account: &AccountId,
    ) -> Result<Vec<RecoveryCodeHash>, SecretsError> {
        Ok(self
            .kept()
            .recovery_codes
            .get(account)
            .cloned()
            .unwrap_or_default())
    }

    async fn use_recovery_code(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        code: &RecoveryCodeHash,
    ) -> Result<bool, SecretsError> {
        let mut kept = self.kept();
        let held = kept
            .recovery_codes
            .get(account)
            .is_some_and(|codes| codes.contains(code));
        if !held {
            return Ok(false);
        }
        self.undo_second_factor_on_rollback(transaction, &kept, account)?;
        if let Some(codes) = kept.recovery_codes.get_mut(account) {
            codes.retain(|kept| kept != code);
        }

        Ok(true)
    }

    async fn forget_second_factor(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        self.undo_second_factor_on_rollback(transaction, &kept, account)?;
        kept.second_factors.remove(account);
        kept.recovery_codes.remove(account);

        Ok(())
    }
}
