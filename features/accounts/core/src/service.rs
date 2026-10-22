use std::sync::Arc;

use clock::Clock;
use eventsourcing::{AggregateId, EventSourcingService, EventStore, ServiceError, Standing};
use mailing::{Mail, Mailer, Unsent};
use thiserror::Error;

use crate::account::{Account, AccountCommand, AccountError, AccountEvent};
use crate::code::PendingVerification;
use crate::email::{EmailAddress, InvalidEmailAddress};
use crate::id::AccountId;
use crate::name::{AccountName, InvalidAccountName};
use crate::password::{InvalidPassword, Password, PasswordHash};
use crate::secrets::{Attempt, EmailAddresses, Passwords, SecretsError, Sessions, Verifications};
use crate::session::{Session, SessionToken, Stage};

#[derive(Debug, Error)]
pub enum AccountServiceError {
    #[error(transparent)]
    InvalidEmail(#[from] InvalidEmailAddress),
    #[error(transparent)]
    InvalidName(#[from] InvalidAccountName),
    #[error(transparent)]
    InvalidPassword(#[from] InvalidPassword),
    #[error("this is not the code that was sent")]
    CodeWrong,
    #[error("this code has run out; ask for a new one")]
    CodeExpired,
    #[error("this code was tried too often; ask for a new one")]
    CodeExhausted,
    #[error("this address and password do not sign in")]
    SignInRefused,
    #[error("this address is not verified yet")]
    NotVerifiedYet,
    #[error(transparent)]
    Events(#[from] ServiceError<AccountError>),
    #[error(transparent)]
    Secrets(#[from] SecretsError),
    #[error(transparent)]
    Unsent(#[from] Unsent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedIn {
    pub session: Session,
    pub account: Account,
}

#[derive(Clone)]
pub struct AccountService {
    events: Arc<EventSourcingService<Account>>,
    email_addresses: Arc<dyn EmailAddresses>,
    passwords: Arc<dyn Passwords>,
    verifications: Arc<dyn Verifications>,
    sessions: Arc<dyn Sessions>,
    mailer: Arc<dyn Mailer>,
    clock: Arc<dyn Clock>,
}

impl AccountService {
    pub fn new(
        store: Arc<dyn EventStore<AccountEvent>>,
        email_addresses: Arc<dyn EmailAddresses>,
        passwords: Arc<dyn Passwords>,
        verifications: Arc<dyn Verifications>,
        sessions: Arc<dyn Sessions>,
        mailer: Arc<dyn Mailer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(store, clock.clone())),
            email_addresses,
            passwords,
            verifications,
            sessions,
            mailer,
            clock,
        }
    }

    pub async fn sign_up(
        &self,
        email: &str,
        name: &str,
        password: &str,
    ) -> Result<(), AccountServiceError> {
        let email = EmailAddress::new(email)?;
        let name = AccountName::new(name)?;
        let password = Password::new(password)?;

        let hashed = password.hashed();
        let id = AccountId::generate(self.clock.now());
        match self.email_addresses.claim(&email, &id).await {
            Err(SecretsError::Taken) => return self.tell_already_signed_up(&email).await,
            claimed => claimed?,
        }

        if let Err(refused) = self
            .events
            .begin(
                &AggregateId::from(&id),
                AccountCommand::SignUp(email.clone(), name),
                &id.acting(),
            )
            .await
        {
            self.email_addresses.release(&email, &id).await?;
            return Err(refused.into());
        }
        self.passwords.keep(&id, &hashed).await?;

        self.send_code(&id, &email).await
    }

    pub async fn send_code_again(&self, email: &str) -> Result<(), AccountServiceError> {
        let email = EmailAddress::new(email)?;
        let Some(id) = self.email_addresses.holder(&email).await? else {
            return Ok(());
        };
        if self.get(&id).await?.state.is_verified() {
            return Ok(());
        }

        self.send_code(&id, &email).await
    }

    pub async fn verify(&self, email: &str, code: &str) -> Result<(), AccountServiceError> {
        let email = EmailAddress::new(email)?;
        let Some(id) = self.email_addresses.holder(&email).await? else {
            return Err(AccountServiceError::CodeWrong);
        };
        let pending = match self
            .verifications
            .attempt(&id, PendingVerification::MAX_TRIES)
            .await?
        {
            Attempt::Nothing => return Err(AccountServiceError::CodeWrong),
            Attempt::Exhausted => return Err(AccountServiceError::CodeExhausted),
            Attempt::Counted(pending) => pending,
        };

        if self.clock.now() > pending.until {
            return Err(AccountServiceError::CodeExpired);
        }
        if !pending.code.is(code) {
            return Err(AccountServiceError::CodeWrong);
        }

        let _landed = self
            .events
            .execute(
                &AggregateId::from(&id),
                AccountCommand::Verify,
                &id.acting(),
            )
            .await?;
        self.verifications.close(&id).await?;

        Ok(())
    }

    pub async fn sign_in(
        &self,
        email: &str,
        password: &str,
    ) -> Result<AccountId, AccountServiceError> {
        Ok(self.signed_in_as(email, password).await?.0)
    }

    async fn signed_in_as(
        &self,
        email: &str,
        password: &str,
    ) -> Result<(AccountId, Account), AccountServiceError> {
        let email = EmailAddress::new(email)?;

        let Some((id, account, kept)) = self.signing_in_as(&email).await? else {
            let _same_work = PasswordHash::stand_in().admits(password);
            return Err(AccountServiceError::SignInRefused);
        };
        if !kept.admits(password) {
            return Err(AccountServiceError::SignInRefused);
        }
        if !account.is_verified() {
            return Err(AccountServiceError::NotVerifiedYet);
        }

        Ok((id, account))
    }

    pub async fn begin_session(
        &self,
        email: &str,
        password: &str,
    ) -> Result<(SessionToken, SignedIn), AccountServiceError> {
        let (id, account) = self.signed_in_as(email, password).await?;
        let token = SessionToken::drawn();
        // TODO second factors: a session owes one, Stage::SecondFactorOwed, where the account has one
        let session = Session::begun(id, Stage::SignedIn, self.clock.now());
        self.sessions.open(&token.digest(), &session).await?;

        Ok((token, SignedIn { session, account }))
    }

    pub async fn signed_in(
        &self,
        token: &SessionToken,
    ) -> Result<Option<SignedIn>, AccountServiceError> {
        let digest = token.digest();
        let Some(session) = self.sessions.find(&digest).await? else {
            return Ok(None);
        };
        if session.is_over(self.clock.now()) {
            self.sessions.end(&digest).await?;
            return Ok(None);
        }
        let account = self.get(&session.account).await?.state;

        Ok(Some(SignedIn { session, account }))
    }

    pub async fn end_session(&self, token: &SessionToken) -> Result<(), AccountServiceError> {
        Ok(self.sessions.end(&token.digest()).await?)
    }

    async fn signing_in_as(
        &self,
        email: &EmailAddress,
    ) -> Result<Option<(AccountId, Account, PasswordHash)>, AccountServiceError> {
        let Some(id) = self.email_addresses.holder(email).await? else {
            return Ok(None);
        };
        let account = match self.get(&id).await {
            Ok(standing) => standing.state,
            Err(AccountServiceError::Events(ServiceError::NotFound { .. })) => return Ok(None),
            Err(failed) => return Err(failed),
        };
        if account.email() != email {
            return Ok(None);
        }
        let Some(kept) = self.passwords.of(&id).await? else {
            return Ok(None);
        };

        Ok(Some((id, account, kept)))
    }

    pub async fn get(&self, id: &AccountId) -> Result<Standing<Account>, AccountServiceError> {
        Ok(self.events.latest(&AggregateId::from(id)).await?)
    }

    async fn send_code(
        &self,
        id: &AccountId,
        email: &EmailAddress,
    ) -> Result<(), AccountServiceError> {
        let pending = PendingVerification::issued(self.clock.now());
        self.verifications.issue(id, &pending).await?;

        Ok(self
            .mailer
            .send(Mail {
                to: email.to_string(),
                subject: "Your Weaveling code".to_owned(),
                body: format!(
                    "{} is your code to confirm this address for Weaveling. It is good for 15 minutes.",
                    pending.code.as_str()
                ),
            })
            .await?)
    }

    async fn tell_already_signed_up(
        &self,
        email: &EmailAddress,
    ) -> Result<(), AccountServiceError> {
        Ok(self
            .mailer
            .send(Mail {
                to: email.to_string(),
                subject: "Signing up to Weaveling".to_owned(),
                body: "Someone tried to sign up to Weaveling with this address, which already has an account. If it was you, sign in instead; if not, you can ignore this mail.".to_owned(),
            })
            .await?)
    }
}
