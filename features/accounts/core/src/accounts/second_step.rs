use eventsourcing::AggregateId;
use transacting::Transaction;

use super::{AccountsCore, SignedIn};
use crate::account::AccountCommand;
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::recovery_code::RecoveryCode;
use crate::session::{SessionToken, Stage};
use crate::throttle::Guarded;

impl AccountsCore {
    /// Answers the second factor a session owes with a code of the app, opening a session signed in all the way in its place.
    pub async fn answer_with_code(
        &self,
        token: &SessionToken,
        code: &str,
    ) -> Result<(SessionToken, SignedIn), AccountsError> {
        let account = self.owing(token).await?;
        let key = self.totp_key()?;
        let who = account.to_string();
        self.admitted(Guarded::CheckingSecondFactor, &who).await?;
        let Some(kept) = self.second_factors.second_factor_of(&account).await? else {
            return Err(AccountsError::SecondFactorCodeWrong);
        };
        let Some(secret) = key.unseal(&kept.secret, &account) else {
            return Err(AccountsError::SecondFactorCodeWrong);
        };
        let Some(step) = secret.accepts(code, self.clock.now(), Some(kept.last_accepted)) else {
            return Err(AccountsError::SecondFactorCodeWrong);
        };

        let answered = transacting::retrying!(&*self.transactions, |transaction| {
            if !self
                .second_factors
                .accept_step(transaction, &account, step)
                .await?
            {
                return Err(AccountsError::SecondFactorCodeWrong);
            }

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
