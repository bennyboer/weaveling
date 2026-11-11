use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::Hash;
use std::mem;
use std::sync::{Arc, Mutex, MutexGuard};

use accounts_core::{
    AccountId, Attempt, CodeDigest, CodePurpose, Dismissal, EmailAddress, EmailAddresses,
    InvitationTicket, InvitationTickets, KeptTotp, KeyCredential, KeyCredentials, KeyId, Operators,
    PasswordHash, Passwords, PendingVerification, RecoveryCodeHash, SealedTotpSecret,
    SecondFactors, SecondStepWay, SecretsError, Session, Sessions, TokenDigest, TotpStep,
    Verifications,
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
    operator_code: Option<(TokenDigest, Option<AccountId>)>,
    operators: HashSet<AccountId>,
    invitation_tickets: HashMap<String, (TokenDigest, InvitationTicket)>,
    ended_invitations: HashSet<String>,
    totps: HashMap<AccountId, KeptTotpSecrets>,
    recovery_codes: HashMap<AccountId, Vec<RecoveryCodeHash>>,
    key_credentials: HashMap<AccountId, BTreeMap<KeyId, KeyCredential>>,
    answered_last_by: HashMap<AccountId, SecondStepWay>,
}

#[derive(Clone, Default)]
struct KeptTotpSecrets {
    in_use: Option<KeptTotp>,
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

    fn undo_totp_on_rollback(
        &self,
        transaction: &mut Transaction,
        kept: &Kept,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        let holder = *account;
        let factor = kept.totps.get(account).cloned();
        let codes = kept.recovery_codes.get(account).cloned();

        self.undo_on_rollback(transaction, move |kept| {
            restore(&mut kept.totps, holder, factor);
            restore(&mut kept.recovery_codes, holder, codes);
        })
    }
}

impl InMemorySecrets {
    fn undo_credentials_on_rollback(
        &self,
        transaction: &mut Transaction,
        kept: &Kept,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        let holder = *account;
        let credentials = kept.key_credentials.get(account).cloned();

        self.undo_on_rollback(transaction, move |kept| {
            restore(&mut kept.key_credentials, holder, credentials);
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
    async fn forget_password_of(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        let forgotten = self.kept().passwords.remove(account);
        let holder = *account;
        self.undo_on_rollback(transaction, move |kept| {
            restore(&mut kept.passwords, holder, forgotten);
        })
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
    async fn end_every_session_of(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        let (ended, open): (HashMap<_, _>, HashMap<_, _>) = mem::take(&mut kept.sessions)
            .into_iter()
            .partition(|(_, session)| session.account == *account);
        kept.sessions = open;
        self.undo_on_rollback(transaction, move |kept| kept.sessions.extend(ended))
    }

    async fn forget_sessions_run_out_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, SecretsError> {
        let mut kept = self.kept();
        let mut run_out: Vec<(OffsetDateTime, TokenDigest)> = kept
            .sessions
            .iter()
            .filter(|(_, session)| session.until < before)
            .map(|(digest, session)| (session.until, digest.clone()))
            .collect();
        run_out.sort_by_key(|(until, _)| *until);
        run_out.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));
        for (_, digest) in &run_out {
            kept.sessions.remove(digest);
        }

        Ok(u64::try_from(run_out.len()).unwrap_or(u64::MAX))
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
        digest: &CodeDigest,
    ) -> Result<bool, SecretsError> {
        let key = (*account, purpose);
        let closed = {
            let mut kept = self.kept();
            match kept.pending.get(&key) {
                Some(pending) if pending.digest == *digest => kept.pending.remove(&key),
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
    async fn forget_codes_of(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        let (forgotten, pending): (HashMap<_, _>, HashMap<_, _>) = mem::take(&mut kept.pending)
            .into_iter()
            .partition(|((holder, _), _)| holder == account);
        kept.pending = pending;
        self.undo_on_rollback(transaction, move |kept| kept.pending.extend(forgotten))
    }

    async fn forget_codes_run_out_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, SecretsError> {
        let mut kept = self.kept();
        let mut run_out: Vec<(OffsetDateTime, (AccountId, CodePurpose))> = kept
            .pending
            .iter()
            .filter(|(_, pending)| pending.until < before)
            .map(|(issued, pending)| (pending.until, *issued))
            .collect();
        run_out.sort_by_key(|(until, _)| *until);
        run_out.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));
        for (_, issued) in &run_out {
            kept.pending.remove(issued);
        }

        Ok(u64::try_from(run_out.len()).unwrap_or(u64::MAX))
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
        let replaced = kept.operator_code.replace((digest.clone(), None));
        drop(kept);
        self.undo_on_rollback(transaction, move |kept| {
            kept.operator_code = replaced;
        })?;

        Ok(true)
    }

    async fn holds_code(&self, digest: &TokenDigest) -> Result<bool, SecretsError> {
        Ok(self
            .kept()
            .operator_code
            .as_ref()
            .is_some_and(|(kept, _)| kept == digest))
    }

    async fn hold_code(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
        account: &AccountId,
    ) -> Result<bool, SecretsError> {
        let mut kept = self.kept();
        if !kept.operators.is_empty() {
            return Ok(false);
        }
        let Some((kept_digest, held_for)) = kept.operator_code.as_mut() else {
            return Ok(false);
        };
        if kept_digest != digest || held_for.is_some() {
            return Ok(false);
        }
        *held_for = Some(*account);
        drop(kept);
        self.undo_on_rollback(transaction, move |kept| {
            if let Some((_, held_for)) = kept.operator_code.as_mut() {
                *held_for = None;
            }
        })?;

        Ok(true)
    }

    async fn take_code(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
        account: &AccountId,
    ) -> Result<bool, SecretsError> {
        let mut kept = self.kept();
        let takes = kept.operators.is_empty()
            && kept.operator_code.as_ref().is_some_and(|(kept, held_for)| {
                kept == digest && held_for.is_none_or(|held_for| held_for == *account)
            });
        if !takes {
            return Ok(false);
        }
        let taken = kept.operator_code.take();
        self.undo_on_rollback(transaction, move |kept| {
            kept.operator_code = taken;
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
        self.undo_totp_on_rollback(transaction, &kept, account)?;
        kept.totps.entry(*account).or_default().being_set_up = Some(secret.clone());

        Ok(())
    }

    async fn totp_being_set_up(
        &self,
        account: &AccountId,
    ) -> Result<Option<SealedTotpSecret>, SecretsError> {
        Ok(self
            .kept()
            .totps
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
            .totps
            .get(account)
            .is_some_and(|factor| factor.being_set_up.as_ref() == Some(being_set_up));
        if !still_being_set_up {
            return Ok(false);
        }
        self.undo_totp_on_rollback(transaction, &kept, account)?;
        kept.totps.insert(
            *account,
            KeptTotpSecrets {
                in_use: Some(KeptTotp {
                    secret: being_set_up.clone(),
                    last_accepted: accepted,
                }),
                being_set_up: None,
            },
        );

        Ok(true)
    }

    async fn totp_of(&self, account: &AccountId) -> Result<Option<KeptTotp>, SecretsError> {
        Ok(self
            .kept()
            .totps
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
            .totps
            .get(account)
            .and_then(|factor| factor.in_use.as_ref())
            .is_some_and(|in_use| step > in_use.last_accepted);
        if !later {
            return Ok(false);
        }
        self.undo_totp_on_rollback(transaction, &kept, account)?;
        if let Some(in_use) = kept
            .totps
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
        self.undo_totp_on_rollback(transaction, &kept, account)?;
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
        self.undo_totp_on_rollback(transaction, &kept, account)?;
        if let Some(codes) = kept.recovery_codes.get_mut(account) {
            codes.retain(|kept| kept != code);
        }

        Ok(true)
    }

    async fn forget_totp(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        self.undo_totp_on_rollback(transaction, &kept, account)?;
        kept.totps.remove(account);

        Ok(())
    }

    async fn forget_recovery_codes(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        self.undo_totp_on_rollback(transaction, &kept, account)?;
        kept.recovery_codes.remove(account);

        Ok(())
    }

    async fn note_answered_by(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        way: SecondStepWay,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        let holder = *account;
        let before = kept.answered_last_by.get(account).copied();
        self.undo_on_rollback(transaction, move |kept| {
            restore(&mut kept.answered_last_by, holder, before);
        })?;
        kept.answered_last_by.insert(*account, way);

        Ok(())
    }

    async fn answered_last_by(
        &self,
        account: &AccountId,
    ) -> Result<Option<SecondStepWay>, SecretsError> {
        Ok(self.kept().answered_last_by.get(account).copied())
    }
    async fn forget_second_factors_of(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        self.undo_totp_on_rollback(transaction, &kept, account)?;
        let holder = *account;
        let answered = kept.answered_last_by.remove(account);
        self.undo_on_rollback(transaction, move |kept| {
            restore(&mut kept.answered_last_by, holder, answered);
        })?;
        kept.totps.remove(account);
        kept.recovery_codes.remove(account);

        Ok(())
    }
}

#[async_trait]
impl KeyCredentials for InMemorySecrets {
    async fn keep_credential(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        key: &KeyId,
        credential: &KeyCredential,
    ) -> Result<bool, SecretsError> {
        let mut kept = self.kept();
        let held = kept
            .key_credentials
            .values()
            .flat_map(BTreeMap::values)
            .any(|held| held.id == credential.id);
        if held {
            return Ok(false);
        }
        self.undo_credentials_on_rollback(transaction, &kept, account)?;
        kept.key_credentials
            .entry(*account)
            .or_default()
            .insert(*key, credential.clone());

        Ok(true)
    }

    async fn credentials_of(
        &self,
        account: &AccountId,
    ) -> Result<Vec<(KeyId, KeyCredential)>, SecretsError> {
        Ok(self
            .kept()
            .key_credentials
            .get(account)
            .map(|held| {
                held.iter()
                    .map(|(key, credential)| (*key, credential.clone()))
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn renew_credential(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        key: &KeyId,
        written: &str,
    ) -> Result<bool, SecretsError> {
        let mut kept = self.kept();
        let held = kept
            .key_credentials
            .get(account)
            .is_some_and(|held| held.contains_key(key));
        if !held {
            return Ok(false);
        }
        self.undo_credentials_on_rollback(transaction, &kept, account)?;
        if let Some(credential) = kept
            .key_credentials
            .get_mut(account)
            .and_then(|held| held.get_mut(key))
        {
            written.clone_into(&mut credential.written);
        }

        Ok(true)
    }

    async fn forget_credential(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        key: &KeyId,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        self.undo_credentials_on_rollback(transaction, &kept, account)?;
        if let Some(held) = kept.key_credentials.get_mut(account) {
            held.remove(key);
        }

        Ok(())
    }
    async fn forget_credentials_of(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        self.undo_credentials_on_rollback(transaction, &kept, account)?;
        kept.key_credentials.remove(account);

        Ok(())
    }
}
