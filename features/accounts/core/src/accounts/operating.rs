use std::collections::BTreeMap;

use eventsourcing::{Agent, Version};
use transacting::Transaction;

use super::AccountsCore;
use crate::account::AccountCommand;
use crate::catalog::AccountSummary;
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::key::{Key, KeyId};
use crate::lock::{LockKind, LockReason};
use crate::operator::OperatorCode;
use crate::second_factor::Totp;
use crate::secrets::Dismissal;

/// The ways an account can sign in, as an operator sees them.
/// It holds names and dates, never a secret.
/// The password is not listed, because every account always has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaysIn {
    pub version: Version,
    pub keys: BTreeMap<KeyId, Key>,
    pub totp: Option<Totp>,
    pub recovery_codes_left: Option<usize>,
}

impl AccountsCore {
    pub async fn draw_operator_code(&self) -> Result<Option<OperatorCode>, AccountsError> {
        let code = OperatorCode::drawn();
        let kept = transacting::retrying!(&*self.transactions, |transaction| {
            Ok::<_, AccountsError>(
                self.operators
                    .keep_code(transaction, &code.digest())
                    .await?,
            )
        })?;

        Ok(kept.then_some(code))
    }

    pub async fn become_operator(&self, id: &AccountId, code: &str) -> Result<(), AccountsError> {
        if !self.get(id).await?.state.is_verified() {
            return Err(AccountsError::NotVerifiedYet);
        }
        let digest = OperatorCode::presented(code).digest();

        transacting::retrying!(&*self.transactions, |transaction| {
            if !self.operators.take_code(transaction, &digest, id).await? {
                return Err(AccountsError::OperatorCodeWrong);
            }
            self.operators.appoint(transaction, id).await?;
            self.commanded_in(
                transaction,
                id,
                None,
                AccountCommand::BecomeOperator,
                &id.acting(),
            )
            .await
        })
    }

    pub async fn lock(
        &self,
        by: &AccountId,
        account: &AccountId,
        reason: &str,
        expected: Option<Version>,
    ) -> Result<(), AccountsError> {
        transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                self.still_operating(transaction, by).await?;
                let reason = LockReason::new(reason).map_err(AccountsError::InvalidLockReason)?;
                self.commanded_in(
                    transaction,
                    account,
                    expected,
                    AccountCommand::Lock(LockKind::Operator, reason),
                    &by.acting(),
                )
                .await
            }
        )?;
        self.check_sessions();

        Ok(())
    }

    /// Turns off an account's TOTP, for someone who lost both the device and the recovery codes.
    /// Every session of the account ends.
    pub async fn remove_totp(
        &self,
        by: &AccountId,
        account: &AccountId,
        expected: Option<Version>,
    ) -> Result<(), AccountsError> {
        transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                self.still_operating(transaction, by).await?;
                let standing = self.get(account).await?;
                self.forgetting_totp(transaction, account, &standing.state)
                    .await?;
                self.commanded_in(
                    transaction,
                    account,
                    Some(expected.unwrap_or(standing.version)),
                    AccountCommand::RemoveTotp,
                    &by.acting(),
                )
                .await
            }
        )?;
        self.check_sessions();

        Ok(())
    }

    pub async fn ways_in(
        &self,
        by: &AccountId,
        account: &AccountId,
    ) -> Result<WaysIn, AccountsError> {
        self.operating(by).await?;
        let standing = self.get(account).await?;

        Ok(WaysIn {
            version: standing.version,
            keys: standing.state.keys().clone(),
            totp: standing.state.totp(),
            recovery_codes_left: standing
                .state
                .second_factor()
                .map(|factor| factor.recovery_codes_left),
        })
    }

    /// Removes a key of an account, for someone who lost it.
    /// Every session of the account ends.
    /// If it was the last second factor, the recovery codes are deleted too.
    pub async fn remove_key_of(
        &self,
        by: &AccountId,
        account: &AccountId,
        key: &KeyId,
        expected: Option<Version>,
    ) -> Result<(), AccountsError> {
        transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                self.still_operating(transaction, by).await?;
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
                    Some(expected.unwrap_or(standing.version)),
                    AccountCommand::RemoveKey { key: *key },
                    &by.acting(),
                )
                .await
            }
        )?;
        self.check_sessions();

        Ok(())
    }

    pub async fn unlock(
        &self,
        by: &AccountId,
        account: &AccountId,
        expected: Option<Version>,
    ) -> Result<(), AccountsError> {
        transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                self.still_operating(transaction, by).await?;
                self.commanded_in(
                    transaction,
                    account,
                    expected,
                    AccountCommand::Unlock(LockKind::Operator),
                    &by.acting(),
                )
                .await
            }
        )
    }

    pub async fn list(
        &self,
        by: &AccountId,
        after: Option<&AccountId>,
        at_most: usize,
    ) -> Result<Vec<AccountSummary>, AccountsError> {
        self.operating(by).await?;

        Ok(self.catalog.listed_after(after, at_most).await?)
    }

    pub async fn appoint_operator(
        &self,
        by: &AccountId,
        account: &AccountId,
        expected: Option<Version>,
    ) -> Result<(), AccountsError> {
        transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                self.still_operating(transaction, by).await?;
                self.operators.appoint(transaction, account).await?;
                self.commanded_in(
                    transaction,
                    account,
                    expected,
                    AccountCommand::BecomeOperator,
                    &by.acting(),
                )
                .await
            }
        )
    }

    pub async fn dismiss_operator(
        &self,
        by: &AccountId,
        account: &AccountId,
        expected: Option<Version>,
    ) -> Result<(), AccountsError> {
        transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                self.still_operating(transaction, by).await?;
                if self.operators.dismiss(transaction, account).await? == Dismissal::LastOne {
                    return Err(AccountsError::LastOperator);
                }
                self.commanded_in(
                    transaction,
                    account,
                    expected,
                    AccountCommand::StopBeingOperator,
                    &by.acting(),
                )
                .await
            }
        )
    }

    pub(super) async fn still_operating(
        &self,
        transaction: &mut Transaction,
        by: &AccountId,
    ) -> Result<(), AccountsError> {
        if self.operators.is_appointed(transaction, by).await? {
            Ok(())
        } else {
            Err(AccountsError::NotAnOperator)
        }
    }

    /// Whether the agent is an account that operates this instance right now.
    /// Any other agent is never an operator.
    pub async fn operates(&self, agent: &Agent) -> Result<bool, AccountsError> {
        let Agent::Account(account) = agent else {
            return Ok(false);
        };
        let Ok(account) = account.as_str().parse::<AccountId>() else {
            return Ok(false);
        };

        Ok(self.get(&account).await?.state.is_operator())
    }

    pub(super) async fn operating(&self, by: &AccountId) -> Result<(), AccountsError> {
        if self.get(by).await?.state.is_operator() {
            Ok(())
        } else {
            Err(AccountsError::NotAnOperator)
        }
    }
}
