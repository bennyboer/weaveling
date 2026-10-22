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
use crate::password::{InvalidPassword, Password};
use crate::secrets::{Credentials, SecretsError, Verifications};

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
    #[error(transparent)]
    Events(#[from] ServiceError<AccountError>),
    #[error(transparent)]
    Secrets(#[from] SecretsError),
    #[error(transparent)]
    Unsent(#[from] Unsent),
}

#[derive(Clone)]
pub struct AccountService {
    events: Arc<EventSourcingService<Account>>,
    credentials: Arc<dyn Credentials>,
    verifications: Arc<dyn Verifications>,
    mailer: Arc<dyn Mailer>,
    clock: Arc<dyn Clock>,
}

impl AccountService {
    pub fn new(
        store: Arc<dyn EventStore<AccountEvent>>,
        credentials: Arc<dyn Credentials>,
        verifications: Arc<dyn Verifications>,
        mailer: Arc<dyn Mailer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(store, clock.clone())),
            credentials,
            verifications,
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

        if self.credentials.registered(&email).await?.is_some() {
            return self.tell_already_signed_up(&email).await;
        }

        let id = AccountId::generate(self.clock.now());
        let _landed = self
            .events
            .begin(
                &AggregateId::from(&id),
                AccountCommand::SignUp(email.clone(), name),
                &id.acting(),
            )
            .await?;
        match self
            .credentials
            .register(&email, &id, &password.hashed())
            .await
        {
            Err(SecretsError::Taken) => return self.tell_already_signed_up(&email).await,
            registered => registered?,
        }

        self.send_code(&id, &email).await
    }

    pub async fn send_code_again(&self, email: &str) -> Result<(), AccountServiceError> {
        let email = EmailAddress::new(email)?;
        let Some(registered) = self.credentials.registered(&email).await? else {
            return Ok(());
        };
        if self.get(&registered.account).await?.state.is_verified() {
            return Ok(());
        }

        self.send_code(&registered.account, &email).await
    }

    pub async fn verify(&self, email: &str, code: &str) -> Result<(), AccountServiceError> {
        let email = EmailAddress::new(email)?;
        let Some(registered) = self.credentials.registered(&email).await? else {
            return Err(AccountServiceError::CodeWrong);
        };
        let id = registered.account;
        let Some(pending) = self.verifications.pending(&id).await? else {
            return Err(AccountServiceError::CodeWrong);
        };

        if pending.failed >= PendingVerification::MAX_FAILED {
            return Err(AccountServiceError::CodeExhausted);
        }
        if self.clock.now() > pending.until {
            return Err(AccountServiceError::CodeExpired);
        }
        if !pending.code.is(code) {
            self.verifications.failed(&id).await?;
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
