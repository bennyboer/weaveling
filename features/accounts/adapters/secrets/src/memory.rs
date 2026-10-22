use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use accounts_core::{
    AccountId, Attempt, Credentials, EmailAddress, PasswordHash, PendingVerification, Registered,
    SecretsError, Verifications,
};
use async_trait::async_trait;

#[derive(Default)]
struct Kept {
    registered: HashMap<EmailAddress, Registered>,
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
impl Credentials for InMemorySecrets {
    async fn register(
        &self,
        email: &EmailAddress,
        account: &AccountId,
        password: &PasswordHash,
    ) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        if kept.registered.contains_key(email) {
            return Err(SecretsError::Taken);
        }
        kept.registered.insert(
            email.clone(),
            Registered {
                account: *account,
                password: password.clone(),
            },
        );

        Ok(())
    }

    async fn registered(&self, email: &EmailAddress) -> Result<Option<Registered>, SecretsError> {
        Ok(self.kept().registered.get(email).cloned())
    }

    async fn release(&self, email: &EmailAddress, account: &AccountId) -> Result<(), SecretsError> {
        let mut kept = self.kept();
        if kept
            .registered
            .get(email)
            .is_some_and(|registered| registered.account == *account)
        {
            kept.registered.remove(email);
        }

        Ok(())
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
