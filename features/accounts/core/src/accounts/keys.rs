use std::collections::BTreeMap;

use eventsourcing::ServiceError;
use serde_json::Value;

use super::AccountsCore;
use crate::account::{AccountCommand, AccountError};
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::key::{Key, KeyId, KeyName, KeyRole};
use crate::key_ceremonies::{CeremonyFailed, KeyCeremonies};
use crate::recovery_code::RecoveryCode;
use crate::session::{SessionToken, Stage};
use crate::throttle::Guarded;

/// A key that was just added.
/// It holds the key's id, and the new session that replaces the one that added it.
/// It also holds the recovery codes, if the key is the account's first second factor.
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

    /// Returns what the browser is asked, to make a key in the given role.
    /// The role is either a passkey that signs in alone, or a key that answers the second step.
    /// It needs the account's password, because an added key is a way to sign in.
    pub async fn begin_adding_key(
        &self,
        account: &AccountId,
        role: KeyRole,
        replacing: Option<&KeyId>,
        password: &str,
    ) -> Result<Value, AccountsError> {
        let ceremonies = self.key_ceremonies()?;
        self.password_confirmed(account, password).await?;
        let holder = self.get(account).await?.state;
        let held: Vec<_> = self
            .key_credentials
            .credentials_of(account)
            .await?
            .into_iter()
            .filter(|(key, _)| Some(key) != replacing)
            .map(|(_, credential)| credential)
            .collect();

        Ok(ceremonies.begin_adding(
            account,
            role,
            holder.email().as_str(),
            holder.name().as_str(),
            &held,
            self.clock.now(),
        ))
    }

    /// Adds the key the browser made, under the given name.
    /// Every other session ends, and this session is replaced by a new one.
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
        let (role, credential) = ceremonies
            .finish_adding(account, answer, self.clock.now())
            .map_err(|refused| match refused {
                CeremonyFailed::NothingUnderWay => AccountsError::NoKeyBeingAdded,
                CeremonyFailed::Refused => AccountsError::KeyRefused,
            })?;
        let key = KeyId::generate(self.clock.now());
        let (codes, hashes) = RecoveryCode::drawn_and_hashed().await;

        let (first, (token, _)) = transacting::retrying!(&*self.transactions, |transaction| {
            let standing = self.get(account).await?;
            let first = role == KeyRole::SecondFactor && standing.state.second_factor().is_none();
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
                    role,
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

    /// Removes a key, which needs the account's password.
    /// Every other session ends, and this session is replaced by a new one.
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
            if !standing.state.answers_second_step_without_key(key) {
                self.second_factors
                    .forget_recovery_codes(transaction, account)
                    .await?;
            }
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

    /// Replaces a key of the account with the passkey the browser just made, keeping the key's name.
    /// It needs the account's password.
    /// This is how a key that answers the second step becomes a passkey: it is made again as a passkey.
    /// If the old key was the last second factor, the second step ends, and its recovery codes are deleted.
    pub async fn replace_key_by_passkey(
        &self,
        account: &AccountId,
        replaced: &KeyId,
        answer: &Value,
        password: &str,
        session: &SessionToken,
    ) -> Result<AddedKey, AccountsError> {
        let ceremonies = self.key_ceremonies()?;
        self.password_confirmed(account, password).await?;
        let name = match self.get(account).await?.state.keys().get(replaced) {
            Some(held) => held.name.clone(),
            None => return Err(AccountsError::NoSuchKey),
        };
        let (role, credential) = ceremonies
            .finish_adding(account, answer, self.clock.now())
            .map_err(|refused| match refused {
                CeremonyFailed::NothingUnderWay => AccountsError::NoKeyBeingAdded,
                CeremonyFailed::Refused => AccountsError::KeyRefused,
            })?;
        if role != KeyRole::Primary {
            return Err(AccountsError::KeyRefused);
        }
        let key = KeyId::generate(self.clock.now());

        let (token, _) = transacting::retrying!(&*self.transactions, |transaction| {
            let standing = self.get(account).await?;
            if !standing.state.keys().contains_key(replaced) {
                return Err(AccountsError::NoSuchKey);
            }
            self.key_credentials
                .forget_credential(transaction, account, replaced)
                .await?;
            if !self
                .key_credentials
                .keep_credential(transaction, account, &key, &credential)
                .await?
            {
                return Err(AccountsError::KeyHeldAlready);
            }
            if !standing.state.answers_second_step_without_key(replaced) {
                self.second_factors
                    .forget_recovery_codes(transaction, account)
                    .await?;
            }
            self.commanded_in(
                transaction,
                account,
                Some(standing.version),
                AccountCommand::RemoveKey { key: *replaced },
                &account.acting(),
            )
            .await?;
            self.commanded_in(
                transaction,
                account,
                None,
                AccountCommand::AddKey {
                    key,
                    name: name.clone(),
                    role,
                },
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

        Ok(AddedKey {
            key,
            session: token,
            recovery_codes: None,
        })
    }

    /// Lets a passkey answer the second step, instead of signing in alone.
    /// It needs the account's password.
    /// Any passkey is able to answer the second step.
    /// If it becomes the account's first second factor, new recovery codes are created.
    /// Every other session ends.
    pub async fn demote_key(
        &self,
        account: &AccountId,
        key: &KeyId,
        password: &str,
        session: &SessionToken,
    ) -> Result<AddedKey, AccountsError> {
        self.password_confirmed(account, password).await?;
        let (codes, hashes) = RecoveryCode::drawn_and_hashed().await;

        let (first, (token, _)) = transacting::retrying!(&*self.transactions, |transaction| {
            let standing = self.get(account).await?;
            let Some(held) = standing.state.keys().get(key) else {
                return Err(AccountsError::NoSuchKey);
            };
            let first = held.role == KeyRole::Primary && standing.state.second_factor().is_none();
            self.commanded_in(
                transaction,
                account,
                Some(standing.version),
                AccountCommand::DemoteKey { key: *key },
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
            key: *key,
            session: token,
            recovery_codes: first.then_some(codes),
        })
    }

    /// Whether the password belongs to the account.
    /// Tries are throttled per account, the same way as when signing in.
    pub(super) async fn password_confirmed(
        &self,
        account: &AccountId,
        password: &str,
    ) -> Result<(), AccountsError> {
        let who = account.to_string();
        self.admitted(Guarded::ConfirmingPassword, &who).await?;
        let confirmed = match self.passwords.of(account).await? {
            Some(kept) => kept.admits_aside(password).await,
            None => false,
        };
        if !confirmed {
            return Err(AccountsError::PasswordWrong);
        }
        self.throttles
            .clear(Guarded::ConfirmingPassword.as_str(), &who)
            .await?;

        Ok(())
    }

    pub(super) fn key_ceremonies(&self) -> Result<&KeyCeremonies, AccountsError> {
        self.key_ceremonies
            .as_ref()
            .ok_or(AccountsError::KeysUnavailable)
    }
}
