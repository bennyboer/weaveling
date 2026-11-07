use ending::SessionEnd;
use time::OffsetDateTime;
use tokio::sync::watch;
use transacting::Transaction;

use super::AccountsCore;
use crate::account::Account;
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::password::PasswordHash;
use crate::session::{Session, SessionToken, Stage, TokenDigest};
use crate::session_end;
use crate::throttle::Guarded;
use email::EmailAddress;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedIn {
    pub session: Session,
    pub account: Account,
}

impl AccountsCore {
    pub async fn sign_in(&self, email: &str, password: &str) -> Result<AccountId, AccountsError> {
        Ok(self.signed_in_as(email, password).await?.0)
    }

    async fn signed_in_as(
        &self,
        email: &str,
        password: &str,
    ) -> Result<(AccountId, Account, OffsetDateTime), AccountsError> {
        let email = EmailAddress::new(email)?;
        self.admitted(Guarded::SigningIn, email.as_str()).await?;
        let began = self.clock.now();

        let Some((id, account, kept)) = self.signing_in_as(&email).await? else {
            let _same_work = PasswordHash::stand_in().admits(password);
            return Err(AccountsError::SignInRefused);
        };
        if !kept.admits(password) {
            return Err(AccountsError::SignInRefused);
        }
        self.throttles
            .clear(Guarded::SigningIn.as_str(), email.as_str())
            .await?;
        if account.is_locked() {
            return Err(AccountsError::Locked);
        }
        if !account.is_verified() {
            return Err(AccountsError::NotVerifiedYet);
        }

        Ok((id, account, began))
    }

    pub async fn begin_session(
        &self,
        email: &str,
        password: &str,
    ) -> Result<(SessionToken, SignedIn), AccountsError> {
        let (id, account, began) = self.signed_in_as(email, password).await?;
        let stage = match account.second_factor() {
            Some(_) => Stage::SecondFactorOwed,
            None => Stage::SignedIn,
        };

        transacting::retrying!(&*self.transactions, |transaction| {
            self.session_opened_within(transaction, id, account.clone(), stage, began)
                .await
        })
    }

    pub(super) async fn session_opened_within(
        &self,
        transaction: &mut Transaction,
        id: AccountId,
        account: Account,
        stage: Stage,
        began: OffsetDateTime,
    ) -> Result<(SessionToken, SignedIn), AccountsError> {
        let token = SessionToken::drawn();
        let session = Session::begun(id, stage, began);
        self.sessions
            .open(transaction, &token.digest(), &session)
            .await?;

        Ok((token, SignedIn { session, account }))
    }

    pub async fn signed_in(&self, token: &SessionToken) -> Result<Option<SignedIn>, AccountsError> {
        let digest = token.digest();
        let Some(session) = self.sessions.find(&digest).await? else {
            return Ok(None);
        };
        if session.is_over(self.clock.now()) {
            self.ended(&digest).await?;
            return Ok(None);
        }
        let account = self.get(&session.account).await?.state;
        if account.is_locked() || !account.counts_session_begun(session.begun) {
            self.ended(&digest).await?;
            return Ok(None);
        }

        Ok(Some(SignedIn { session, account }))
    }

    pub async fn end_session(&self, token: &SessionToken) -> Result<(), AccountsError> {
        self.ended(&token.digest()).await?;
        self.check_sessions();

        Ok(())
    }

    async fn signing_in_as(
        &self,
        email: &EmailAddress,
    ) -> Result<Option<(AccountId, Account, PasswordHash)>, AccountsError> {
        let Some((id, account)) = self.whose(email).await? else {
            return Ok(None);
        };
        let Some(kept) = self.passwords.of(&id).await? else {
            return Ok(None);
        };

        Ok(Some((id, account, kept)))
    }

    pub fn session_end(&self, token: &SessionToken, until: OffsetDateTime) -> SessionEnd {
        session_end::of(self.clone(), SessionToken::presented(token.as_str()), until)
    }

    pub(crate) fn session_checks(&self) -> watch::Receiver<u64> {
        self.sessions_checked.subscribe()
    }

    pub(crate) fn now(&self) -> OffsetDateTime {
        self.clock.now()
    }

    pub(crate) async fn still_acting(&self, token: &SessionToken) -> bool {
        !matches!(self.signed_in(token).await, Ok(None))
    }

    async fn ended(&self, digest: &TokenDigest) -> Result<(), AccountsError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            self.sessions.end(transaction, digest).await?;

            Ok(())
        })
    }

    pub(super) fn check_sessions(&self) {
        self.sessions_checked.send_modify(|checked| *checked += 1);
    }
}
