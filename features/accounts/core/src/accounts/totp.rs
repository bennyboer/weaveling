use super::AccountsCore;
use crate::account::AccountCommand;
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::recovery_code::{RecoveryCode, RecoveryCodeHash};
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

    pub async fn begin_setting_up_totp(
        &self,
        account: &AccountId,
    ) -> Result<SettingUpTotp, AccountsError> {
        let key = self.totp_key()?;
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
        let codes = RecoveryCode::drawn();
        let hashes: Vec<RecoveryCodeHash> = codes.iter().map(RecoveryCode::hashed).collect();
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

    /// Draws new recovery codes in place of the old, for a current code of the app.
    pub async fn draw_recovery_codes(
        &self,
        account: &AccountId,
        code: &str,
    ) -> Result<Vec<RecoveryCode>, AccountsError> {
        let key = self.totp_key()?;
        let who = account.to_string();
        self.admitted(Guarded::CheckingSecondFactor, &who).await?;
        let Some(kept) = self.second_factors.totp_of(account).await? else {
            return Err(AccountsError::NoSecondFactor);
        };
        let Some(secret) = key.unseal(&kept.secret, account) else {
            return Err(AccountsError::TotpCodeWrong);
        };
        let Some(step) = secret.accepts(code, self.clock.now(), Some(kept.last_accepted)) else {
            return Err(AccountsError::TotpCodeWrong);
        };
        let codes = RecoveryCode::drawn();
        let hashes: Vec<RecoveryCodeHash> = codes.iter().map(RecoveryCode::hashed).collect();

        transacting::retrying!(&*self.transactions, |transaction| {
            if !self
                .second_factors
                .accept_step(transaction, account, step)
                .await?
            {
                return Err(AccountsError::TotpCodeWrong);
            }
            self.second_factors
                .keep_recovery_codes(transaction, account, &hashes)
                .await?;
            self.commanded_in(
                transaction,
                account,
                None,
                AccountCommand::DrawRecoveryCodes,
                &account.acting(),
            )
            .await?;

            Ok::<_, AccountsError>(())
        })?;
        self.throttles
            .clear(Guarded::CheckingSecondFactor.as_str(), &who)
            .await?;

        Ok(codes)
    }

    /// Turns TOTP off for a current code of the app, so every other session ends, and this one goes on as a new one.
    pub async fn turn_off_totp(
        &self,
        account: &AccountId,
        code: &str,
        session: &SessionToken,
    ) -> Result<SessionToken, AccountsError> {
        let key = self.totp_key()?;
        let who = account.to_string();
        self.admitted(Guarded::CheckingSecondFactor, &who).await?;
        let Some(kept) = self.second_factors.totp_of(account).await? else {
            return Err(AccountsError::NoSecondFactor);
        };
        let Some(secret) = key.unseal(&kept.secret, account) else {
            return Err(AccountsError::TotpCodeWrong);
        };
        let Some(step) = secret.accepts(code, self.clock.now(), Some(kept.last_accepted)) else {
            return Err(AccountsError::TotpCodeWrong);
        };
        let holder = self.get(account).await?.state;

        let (token, _) = transacting::retrying!(&*self.transactions, |transaction| {
            if !self
                .second_factors
                .accept_step(transaction, account, step)
                .await?
            {
                return Err(AccountsError::TotpCodeWrong);
            }
            self.second_factors
                .forget_totp(transaction, account)
                .await?;
            self.commanded_in(
                transaction,
                account,
                None,
                AccountCommand::RemoveTotp,
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

        Ok(token)
    }

    pub(super) fn totp_key(&self) -> Result<&TotpKey, AccountsError> {
        self.totp_key.as_ref().ok_or(AccountsError::TotpUnavailable)
    }
}
