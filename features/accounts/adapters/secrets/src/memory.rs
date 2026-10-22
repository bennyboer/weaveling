use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use accounts_core::{
    AccountId, Attempt, EmailAddress, EmailAddresses, PasswordHash, Passwords, PendingVerification,
    SecretsError, Verifications,
};
use async_trait::async_trait;

#[derive(Default)]
struct Kept {
    email_addresses: HashMap<EmailAddress, AccountId>,
    passwords: HashMap<AccountId, PasswordHash>,
    pending: HashMap<AccountId, PendingVerification>,
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
impl Verifications for InMemorySecrets {
    async fn issue(
        &self,
        account: &AccountId,
        pending: &PendingVerification,
    ) -> Result<(), SecretsError> {
        self.kept().pending.insert(*account, pending.clone());

        Ok(())
    }

    async fn attempt(&self, account: &AccountId, within: u32) -> Result<Attempt, SecretsError> {
        Ok(match self.kept().pending.get_mut(account) {
            None => Attempt::Nothing,
            Some(pending) if pending.tried >= within => Attempt::Exhausted,
            Some(pending) => {
                pending.tried += 1;
                Attempt::Counted(pending.clone())
            }
        })
    }

    async fn close(&self, account: &AccountId) -> Result<(), SecretsError> {
        self.kept().pending.remove(account);

        Ok(())
    }
}
