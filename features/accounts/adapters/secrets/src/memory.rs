use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use accounts_core::{
    AccountId, Attempt, CodePurpose, EmailAddress, EmailAddresses, Guarded, PasswordHash,
    Passwords, PendingVerification, SecretsError, Session, Sessions, Throttles, TokenDigest, Tries,
    Verifications,
};
use async_trait::async_trait;
use time::OffsetDateTime;

#[derive(Default)]
struct Kept {
    email_addresses: HashMap<EmailAddress, AccountId>,
    passwords: HashMap<AccountId, PasswordHash>,
    pending: HashMap<(AccountId, CodePurpose), PendingVerification>,
    sessions: HashMap<TokenDigest, Session>,
    tries: HashMap<(Guarded, EmailAddress), Tries>,
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
    async fn claim(&self, email: &EmailAddress, account: &AccountId) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        if kept.email_addresses.contains_key(email) {
            return Err(SecretsError::Taken);
        }
        kept.email_addresses.insert(email.clone(), *account);

        Ok(())
    }

    async fn holder(&self, email: &EmailAddress) -> Result<Option<AccountId>, SecretsError> {
        Ok(self.kept().email_addresses.get(email).copied())
    }

    async fn release(&self, email: &EmailAddress, account: &AccountId) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        if kept.email_addresses.get(email) == Some(account) {
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
impl Throttles for InMemorySecrets {
    async fn tries(
        &self,
        guarded: Guarded,
        email: &EmailAddress,
    ) -> Result<Option<Tries>, SecretsError> {
        Ok(self.kept().tries.get(&(guarded, email.clone())).copied())
    }

    async fn count(
        &self,
        guarded: Guarded,
        email: &EmailAddress,
        at: OffsetDateTime,
    ) -> Result<Tries, SecretsError> {
        let mut kept = self.kept();
        let tries = kept.tries.entry((guarded, email.clone())).or_insert(Tries {
            counted: 0,
            last: at,
        });
        tries.counted += 1;
        tries.last = at;

        Ok(*tries)
    }

    async fn clear(&self, guarded: Guarded, email: &EmailAddress) -> Result<(), SecretsError> {
        self.kept().tries.remove(&(guarded, email.clone()));

        Ok(())
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
