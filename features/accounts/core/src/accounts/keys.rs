use std::collections::BTreeMap;

use eventsourcing::ServiceError;
use serde_json::Value;

use super::AccountsCore;
use crate::account::{AccountCommand, AccountError};
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::key::{Key, KeyId, KeyName, KeyRole};
use crate::key_ceremonies::{KeyCeremonies, KeyNotAdded};
use crate::recovery_code::{RecoveryCode, RecoveryCodeHash};
use crate::session::{SessionToken, Stage};
use crate::throttle::Guarded;

/// A key just added: its id, the session going on in place of the one that added it, and the recovery codes, where it is the account's first second factor.
#[derive(Debug)]
pub struct AddedKey {
    pub key: KeyId,
    pub session: SessionToken,
    pub recovery_codes: Option<Vec<RecoveryCode>>,
}

impl AccountsCore {
    pub fn holding_key_ceremonies(self, ceremonies: KeyCeremonies) -> Self {
        Self {
            key_ceremonies: Some(ceremonies),
            ..self
        }
    }

    pub fn keeps_keys(&self) -> bool {
        self.key_ceremonies.is_some()
    }

    pub async fn keys_of(
        &self,
        account: &AccountId,
    ) -> Result<BTreeMap<KeyId, Key>, AccountsError> {
        Ok(self.get(account).await?.state.keys().clone())
    }

    /// What the browser is asked to make a key answering the account's second step with.
    pub async fn begin_adding_key(&self, account: &AccountId) -> Result<Value, AccountsError> {
        let ceremonies = self.key_ceremonies()?;
        let holder = self.get(account).await?.state;
        let held: Vec<_> = self
            .key_credentials
            .credentials_of(account)
            .await?
            .into_iter()
            .map(|(_, credential)| credential)
            .collect();

        Ok(ceremonies.begin_adding(
            account,
            holder.email().as_str(),
            holder.name().as_str(),
            &held,
            self.clock.now(),
        ))
    }

    /// Adds the key the browser made, under the name given, so every other session ends, and this one goes on as a new one.
    pub async fn finish_adding_key(
        &self,
        account: &AccountId,
        name: &str,
        answer: &Value,
        session: &SessionToken,
    ) -> Result<AddedKey, AccountsError> {
        let ceremonies = self.key_ceremonies()?;
        let name = KeyName::new(name).map_err(AccountsError::InvalidKeyName)?;
        if self
            .get(account)
            .await?
            .state
            .keys()
            .values()
            .any(|held| held.name == name)
        {
            return Err(AccountsError::Events(ServiceError::Refused(
                AccountError::KeyNameTaken,
            )));
        }
        let credential = ceremonies
            .finish_adding(account, answer, self.clock.now())
            .map_err(|refused| match refused {
                KeyNotAdded::NothingUnderWay => AccountsError::NoKeyBeingAdded,
                KeyNotAdded::Refused => AccountsError::KeyRefused,
            })?;
        let key = KeyId::generate(self.clock.now());
        let codes = RecoveryCode::drawn();
        let hashes: Vec<RecoveryCodeHash> = codes.iter().map(RecoveryCode::hashed).collect();

        let (first, (token, _)) = transacting::retrying!(&*self.transactions, |transaction| {
            let standing = self.get(account).await?;
            let first = standing.state.second_factor().is_none();
            if !self
                .key_credentials
                .keep_credential(transaction, account, &key, &credential)
                .await?
            {
                return Err(AccountsError::KeyHeldAlready);
            }
            self.commanded_in(
                transaction,
                account,
                Some(standing.version),
                AccountCommand::AddKey {
                    key,
                    name: name.clone(),
                    role: KeyRole::SecondFactor,
                },
                &account.acting(),
            )
            .await?;
            if first {
                self.second_factors
                    .keep_recovery_codes(transaction, account, &hashes)
                    .await?;
            }
            self.sessions.end(transaction, &session.digest()).await?;
            let opened = self
                .session_opened_within(
                    transaction,
                    *account,
                    standing.state,
                    Stage::SignedIn,
                    self.clock.now(),
                )
                .await?;

            Ok::<_, AccountsError>((first, opened))
        })?;
        self.check_sessions();

        Ok(AddedKey {
            key,
            session: token,
            recovery_codes: first.then_some(codes),
        })
    }

    pub async fn rename_key(
        &self,
        account: &AccountId,
        key: &KeyId,
        name: &str,
    ) -> Result<(), AccountsError> {
        let name = KeyName::new(name).map_err(AccountsError::InvalidKeyName)?;

        transacting::retrying!(&*self.transactions, |transaction| {
            self.commanded_in(
                transaction,
                account,
                None,
                AccountCommand::RenameKey {
                    key: *key,
                    name: name.clone(),
                },
                &account.acting(),
            )
            .await
        })
    }

    /// Removes a key for the account's password, so every other session ends, and this one goes on as a new one.
    pub async fn remove_key(
        &self,
        account: &AccountId,
        key: &KeyId,
        password: &str,
        session: &SessionToken,
    ) -> Result<SessionToken, AccountsError> {
        self.password_confirmed(account, password).await?;

        let (token, _) = transacting::retrying!(&*self.transactions, |transaction| {
            let standing = self.get(account).await?;
            if !standing.state.keys().contains_key(key) {
                return Err(AccountsError::NoSuchKey);
            }
            self.key_credentials
                .forget_credential(transaction, account, key)
                .await?;
            self.commanded_in(
                transaction,
                account,
                Some(standing.version),
                AccountCommand::RemoveKey { key: *key },
                &account.acting(),
            )
            .await?;
            self.sessions.end(transaction, &session.digest()).await?;

            self.session_opened_within(
                transaction,
                *account,
                standing.state,
                Stage::SignedIn,
                self.clock.now(),
            )
            .await
        })?;
        self.check_sessions();

        Ok(token)
    }

    /// Whether the password is the account's, throttled as signing in is, by the account.
    async fn password_confirmed(
        &self,
        account: &AccountId,
        password: &str,
    ) -> Result<(), AccountsError> {
        let who = account.to_string();
        self.admitted(Guarded::ConfirmingPassword, &who).await?;
        let confirmed = self
            .passwords
            .of(account)
            .await?
            .is_some_and(|kept| kept.admits(password));
        if !confirmed {
            return Err(AccountsError::PasswordWrong);
        }
        self.throttles
            .clear(Guarded::ConfirmingPassword.as_str(), &who)
            .await?;

        Ok(())
    }

    fn key_ceremonies(&self) -> Result<&KeyCeremonies, AccountsError> {
        self.key_ceremonies
            .as_ref()
            .ok_or(AccountsError::KeysUnavailable)
    }
}
