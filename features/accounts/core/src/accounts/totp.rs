use super::AccountsCore;
use transacting::Transaction;

use crate::account::{Account, AccountCommand};
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::recovery_code::RecoveryCode;
use crate::session::{SessionToken, Stage};
use crate::throttle::Guarded;
use crate::totp::TotpSecret;
use crate::totp_key::TotpKey;

const ISSUER: &str = "Weaveling";

/// TOTP being set up, as the author is shown it: the secret to type, and the address its QR code holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingUpTotp {
    pub secret: String,
    pub otpauth_uri: String,
}

impl AccountsCore {
    pub fn sealing_totp_secrets(self, key: TotpKey) -> Self {
        Self {
            totp_key: Some(key),
            ..self
        }
    }

    pub fn keeps_totp(&self) -> bool {
        self.totp_key.is_some()
    }

    /// Begins setting up an app for the second step, for the account's password, as it may replace the app in use and its recovery codes.
    pub async fn begin_setting_up_totp(
        &self,
        account: &AccountId,
        password: &str,
    ) -> Result<SettingUpTotp, AccountsError> {
        let key = self.totp_key()?;
        self.password_confirmed(account, password).await?;
        let holder = self.get(account).await?.state;
        let secret = TotpSecret::drawn();
        let sealed = key.seal(&secret, account);

        transacting::retrying!(&*self.transactions, |transaction| {
            self.second_factors
                .begin_setting_up(transaction, account, &sealed)
                .await?;

            Ok::<_, AccountsError>(())
        })?;

        Ok(SettingUpTotp {
            secret: secret.to_base32(),
            otpauth_uri: secret.otpauth_uri(ISSUER, holder.email().as_str()),
        })
    }

    /// Finishes setting up by a code of the app, so every other session ends, and this one goes on as a new one.
    pub async fn finish_setting_up_totp(
        &self,
        account: &AccountId,
        code: &str,
        session: &SessionToken,
    ) -> Result<(SessionToken, Vec<RecoveryCode>), AccountsError> {
        let key = self.totp_key()?;
        let Some(sealed) = self.second_factors.totp_being_set_up(account).await? else {
            return Err(AccountsError::NothingBeingSetUp);
        };
        let Some(secret) = key.unseal(&sealed, account) else {
            return Err(AccountsError::NothingBeingSetUp);
        };
        let who = account.to_string();
        self.admitted(Guarded::CheckingSecondFactor, &who).await?;
        let Some(step) = secret.accepts(code, self.clock.now(), None) else {
            return Err(AccountsError::TotpCodeWrong);
        };
        let (codes, hashes) = RecoveryCode::drawn_and_hashed().await;
        let holder = self.get(account).await?.state;

        let (token, _) = transacting::retrying!(&*self.transactions, |transaction| {
            if !self
                .second_factors
                .finish_setting_up(transaction, account, &sealed, step)
                .await?
            {
                return Err(AccountsError::NothingBeingSetUp);
            }
            self.second_factors
                .keep_recovery_codes(transaction, account, &hashes)
                .await?;
            self.commanded_in(
                transaction,
                account,
                None,
                AccountCommand::AddTotp,
                &account.acting(),
            )
            .await?;
            self.sessions.end(transaction, &session.digest()).await?;

            self.session_opened_within(
                transaction,
                *account,
                holder.clone(),
                Stage::SignedIn,
                self.clock.now(),
            )
            .await
        })?;
        self.throttles
            .clear(Guarded::CheckingSecondFactor.as_str(), &who)
            .await?;
        self.check_sessions();

        Ok((token, codes))
    }

    /// Draws new recovery codes in place of the old, for the account's password, whatever answers its second step.
    pub async fn draw_recovery_codes(
        &self,
        account: &AccountId,
        password: &str,
    ) -> Result<Vec<RecoveryCode>, AccountsError> {
        self.password_confirmed(account, password).await?;
        let (codes, hashes) = RecoveryCode::drawn_and_hashed().await;

        transacting::retrying!(&*self.transactions, |transaction| {
            self.commanded_in(
                transaction,
                account,
                None,
                AccountCommand::DrawRecoveryCodes,
                &account.acting(),
            )
            .await?;
            self.second_factors
                .keep_recovery_codes(transaction, account, &hashes)
                .await?;

            Ok::<_, AccountsError>(())
        })?;

        Ok(codes)
    }

    /// Turns TOTP off for the account's password, so every other session ends, and this one goes on as a new one; the recovery codes go too where nothing else answers the second step.
    pub async fn turn_off_totp(
        &self,
        account: &AccountId,
        password: &str,
        session: &SessionToken,
    ) -> Result<SessionToken, AccountsError> {
        self.password_confirmed(account, password).await?;

        let (token, _) = transacting::retrying!(&*self.transactions, |transaction| {
            let standing = self.get(account).await?;
            if standing.state.totp().is_none() {
                return Err(AccountsError::NoSecondFactor);
            }
            self.forgetting_totp(transaction, account, &standing.state)
                .await?;
            self.commanded_in(
                transaction,
                account,
                Some(standing.version),
                AccountCommand::RemoveTotp,
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

    /// Forgets the account's TOTP, and its recovery codes where no key answers the second step without it.
    pub(super) async fn forgetting_totp(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        holder: &Account,
    ) -> Result<(), AccountsError> {
        self.second_factors
            .forget_totp(transaction, account)
            .await?;
        if !holder.answers_second_step_without_totp() {
            self.second_factors
                .forget_recovery_codes(transaction, account)
                .await?;
        }

        Ok(())
    }

    pub(super) fn totp_key(&self) -> Result<&TotpKey, AccountsError> {
        self.totp_key.as_ref().ok_or(AccountsError::TotpUnavailable)
    }
}
