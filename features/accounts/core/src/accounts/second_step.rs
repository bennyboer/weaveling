use eventsourcing::AggregateId;
use serde_json::Value;
use transacting::Transaction;

use super::{AccountsCore, SignedIn};
use crate::account::{Account, AccountCommand};
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::key::{KeyId, KeyRole};
use crate::key_ceremonies::CeremonyFailed;
use crate::recovery_code::RecoveryCode;
use crate::second_factor::{PreferredWay, SecondStepWay};
use crate::secrets::KeyCredential;
use crate::session::{SessionToken, Stage};
use crate::throttle::Guarded;

impl AccountsCore {
    /// Answers the second factor a session owes with a code of the app, opening a session signed in all the way in its place.
    pub async fn answer_with_totp(
        &self,
        token: &SessionToken,
        code: &str,
    ) -> Result<(SessionToken, SignedIn), AccountsError> {
        let account = self.owing(token).await?;
        let key = self.totp_key()?;
        let who = account.to_string();
        self.admitted(Guarded::CheckingSecondFactor, &who).await?;
        let Some(kept) = self.second_factors.totp_of(&account).await? else {
            return Err(AccountsError::TotpCodeWrong);
        };
        let Some(secret) = key.unseal(&kept.secret, &account) else {
            return Err(AccountsError::TotpCodeWrong);
        };
        let Some(step) = secret.accepts(code, self.clock.now(), Some(kept.last_accepted)) else {
            return Err(AccountsError::TotpCodeWrong);
        };

        let answered = transacting::retrying!(&*self.transactions, |transaction| {
            if !self
                .second_factors
                .accept_step(transaction, &account, step)
                .await?
            {
                return Err(AccountsError::TotpCodeWrong);
            }
            self.second_factors
                .note_answered_by(transaction, &account, SecondStepWay::Totp)
                .await?;

            self.signed_in_after(transaction, token, &account).await
        })?;
        self.throttles
            .clear(Guarded::CheckingSecondFactor.as_str(), &who)
            .await?;

        Ok(answered)
    }

    /// Answers the second factor a session owes with one of the account's recovery codes, which serves only this once.
    pub async fn answer_with_recovery_code(
        &self,
        token: &SessionToken,
        typed: &str,
    ) -> Result<(SessionToken, SignedIn), AccountsError> {
        let account = self.owing(token).await?;
        let who = account.to_string();
        self.admitted(Guarded::CheckingSecondFactor, &who).await?;
        let Some(presented) = RecoveryCode::presented(typed) else {
            return Err(AccountsError::RecoveryCodeWrong);
        };
        let Some(hash) = self
            .second_factors
            .recovery_codes_of(&account)
            .await?
            .into_iter()
            .find(|hash| hash.admits(&presented))
        else {
            return Err(AccountsError::RecoveryCodeWrong);
        };

        let answered = transacting::retrying!(&*self.transactions, |transaction| {
            if !self
                .second_factors
                .use_recovery_code(transaction, &account, &hash)
                .await?
            {
                return Err(AccountsError::RecoveryCodeWrong);
            }
            self.commanded_in(
                transaction,
                &account,
                None,
                AccountCommand::UseRecoveryCode,
                &account.acting(),
            )
            .await?;

            self.signed_in_after(transaction, token, &account).await
        })?;
        self.throttles
            .clear(Guarded::CheckingSecondFactor.as_str(), &who)
            .await?;

        Ok(answered)
    }

    /// What the browser is asked for a touch of one of the keys answering the account's second step.
    pub async fn begin_answering_with_key(
        &self,
        token: &SessionToken,
    ) -> Result<Value, AccountsError> {
        let account = self.owing(token).await?;
        let ceremonies = self.key_ceremonies()?;
        let held = self.second_step_credentials(&account).await?;
        if held.is_empty() {
            return Err(AccountsError::NoSuchKey);
        }

        ceremonies
            .begin_answering(&account, &held, self.clock.now())
            .map_err(|_| AccountsError::KeyRefused)
    }

    /// Answers the second factor a session owes with a key's touch, opening a session signed in all the way in its place.
    pub async fn answer_with_key(
        &self,
        token: &SessionToken,
        answer: &Value,
    ) -> Result<(SessionToken, SignedIn), AccountsError> {
        let account = self.owing(token).await?;
        let ceremonies = self.key_ceremonies()?;
        let who = account.to_string();
        self.admitted(Guarded::CheckingSecondFactor, &who).await?;
        let held = self.second_step_credentials(&account).await?;
        let (key, renewed) = ceremonies
            .finish_answering(&account, answer, &held, self.clock.now())
            .map_err(|failed| match failed {
                CeremonyFailed::NothingUnderWay => AccountsError::NoKeyBeingAsked,
                CeremonyFailed::Refused => AccountsError::KeyRefused,
            })?;

        let answered = transacting::retrying!(&*self.transactions, |transaction| {
            if !self
                .key_credentials
                .renew_credential(transaction, &account, &key, &renewed)
                .await?
            {
                return Err(AccountsError::KeyRefused);
            }
            self.second_factors
                .note_answered_by(transaction, &account, SecondStepWay::Key)
                .await?;

            self.signed_in_after(transaction, token, &account).await
        })?;
        self.throttles
            .clear(Guarded::CheckingSecondFactor.as_str(), &who)
            .await?;

        Ok(answered)
    }

    /// The credentials of the account's keys that answer its second step, not those signing in alone.
    async fn second_step_credentials(
        &self,
        account: &AccountId,
    ) -> Result<Vec<(KeyId, KeyCredential)>, AccountsError> {
        let holder = self.get(account).await?.state;

        Ok(self
            .key_credentials
            .credentials_of(account)
            .await?
            .into_iter()
            .filter(|(key, _)| {
                holder
                    .keys()
                    .get(key)
                    .is_some_and(|held| held.role == KeyRole::SecondFactor)
            })
            .collect())
    }

    /// The way the account's second step is asked first, as it chose, or as it answered last.
    pub async fn asked_way(
        &self,
        account: &AccountId,
        holder: &Account,
    ) -> Result<Option<SecondStepWay>, AccountsError> {
        if holder.second_factor().is_none() {
            return Ok(None);
        }

        Ok(holder.asked_way(self.second_factors.answered_last_by(account).await?))
    }

    pub async fn choose_preferred_way(
        &self,
        account: &AccountId,
        way: PreferredWay,
    ) -> Result<(), AccountsError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            self.commanded_in(
                transaction,
                account,
                None,
                AccountCommand::ChoosePreferredWay(way),
                &account.acting(),
            )
            .await
        })
    }

    async fn owing(&self, token: &SessionToken) -> Result<AccountId, AccountsError> {
        match self.signed_in(token).await? {
            Some(signed_in) if signed_in.session.stage == Stage::SecondFactorOwed => {
                Ok(signed_in.session.account)
            }
            _ => Err(AccountsError::NoSecondFactorOwed),
        }
    }

    async fn signed_in_after(
        &self,
        transaction: &mut Transaction,
        owing: &SessionToken,
        account: &AccountId,
    ) -> Result<(SessionToken, SignedIn), AccountsError> {
        self.sessions.end(transaction, &owing.digest()).await?;
        let holder = self
            .events
            .latest_within(transaction, &AggregateId::from(account))
            .await?
            .state;

        self.session_opened_within(
            transaction,
            *account,
            holder,
            Stage::SignedIn,
            self.clock.now(),
        )
        .await
    }
}
