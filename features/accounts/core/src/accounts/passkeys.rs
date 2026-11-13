use serde_json::Value;

use super::{AccountsCore, SignedIn};
use crate::error::AccountsError;
use crate::key::KeyRole;
use crate::key_ceremonies::{CeremonyFailed, SigningIn};
use crate::session::SessionToken;

impl AccountsCore {
    /// Starts a sign-in with any passkey the browser holds for this instance, before anybody is named.
    /// Returns the id of the sign-in, and what the browser is asked.
    pub fn begin_signing_in_with_passkey(&self) -> Result<(SigningIn, Value), AccountsError> {
        Ok(self.key_ceremonies()?.begin_signing_in(self.clock.now()))
    }

    /// Signs in with a passkey alone.
    /// The device has already checked who holds the passkey, so no second step is needed.
    pub async fn sign_in_with_passkey(
        &self,
        signing_in: &SigningIn,
        answer: &Value,
    ) -> Result<(SessionToken, SignedIn), AccountsError> {
        let ceremonies = self.key_ceremonies()?;
        let (account, named) = ceremonies
            .named_by(answer)
            .map_err(|_| AccountsError::KeyRefused)?;
        let holder = self
            .get(&account)
            .await
            .map_err(|_| AccountsError::KeyRefused)?
            .state;
        let Some((key, credential)) = self
            .key_credentials
            .credentials_of(&account)
            .await?
            .into_iter()
            .find(|(key, credential)| {
                credential.id == named
                    && holder
                        .keys()
                        .get(key)
                        .is_some_and(|held| held.role == KeyRole::Primary)
            })
        else {
            return Err(AccountsError::KeyRefused);
        };
        let renewed = ceremonies
            .finish_signing_in(signing_in, answer, &credential, self.clock.now())
            .map_err(|failed| match failed {
                CeremonyFailed::NothingUnderWay => AccountsError::NoKeyBeingAsked,
                CeremonyFailed::Refused => AccountsError::KeyRefused,
            })?;
        if holder.is_locked_by_the_operator() {
            return Err(AccountsError::Locked);
        }
        if !holder.is_verified() {
            return Err(AccountsError::NotVerifiedYet);
        }
        let began = self.clock.now();

        transacting::retrying!(&*self.transactions, |transaction| {
            if !self
                .key_credentials
                .renew_credential(transaction, &account, &key, &renewed)
                .await?
            {
                return Err(AccountsError::KeyRefused);
            }

            self.session_opened_within(
                transaction,
                account,
                holder.clone(),
                holder.signs_in_to(),
                began,
            )
            .await
        })
    }
}
