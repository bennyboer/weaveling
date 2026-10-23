use std::sync::Arc;

use clock::Clock;
use eventsourcing::{Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing};
use mailing::{Mail, Mailer, Unsent};
use thiserror::Error;
use time::Duration;

use crate::account::{Account, AccountCommand, AccountError, AccountEvent};
use crate::catalog::{AccountCatalog, AccountSummary, CatalogError};
use crate::code::{CodePurpose, PendingVerification};
use crate::decline::DeclineReason;
use crate::id::AccountId;
use crate::lock::{InvalidLockReason, LockKind, LockReason};
use crate::name::{AccountName, InvalidAccountName};
use crate::operator::OperatorCode;
use crate::password::{InvalidPassword, Password, PasswordHash};
use crate::secrets::{
    Attempt, Dismissal, EmailAddresses, InvitationTickets, Operators, Passwords, SecretStores,
    SecretsError, Sessions, Verifications,
};
use crate::session::{Session, SessionToken, Stage, TokenDigest};
use crate::signing_up::SigningUp;
use crate::throttle::Guarded;
use crate::ticket::{InvitationTicket, InvitationToken};
use email::{EmailAddress, InvalidEmailAddress};
use throttling::{Throttles, ThrottlesError};

const CLAIM_ABANDONED_AFTER: Duration = Duration::minutes(5);

#[derive(Debug, Error)]
pub enum AccountServiceError {
    #[error(transparent)]
    InvalidEmail(#[from] InvalidEmailAddress),
    #[error(transparent)]
    InvalidName(#[from] InvalidAccountName),
    #[error(transparent)]
    InvalidPassword(#[from] InvalidPassword),
    #[error("{0}")]
    InvalidLockReason(InvalidLockReason),
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
    #[error("this account is locked")]
    Locked,
    #[error("only an operator may do this")]
    NotAnOperator,
    #[error("the last operator keeps the role")]
    LastOperator,
    #[error("this is not the code that makes an operator")]
    OperatorCodeWrong,
    #[error("signing up here is by invitation only")]
    SigningUpByInvitationOnly,
    #[error("this invitation's link lets nobody sign up, or not any more")]
    InvitationInvalid,
    #[error("this invitation has run out; ask for a new one")]
    InvitationExpired,
    #[error("this address already has an account; sign in instead")]
    EmailAddressHasAccount,
    #[error("too many tries; wait {wait} before trying again")]
    Throttled { wait: Duration },
    #[error(transparent)]
    Events(#[from] ServiceError<AccountError>),
    #[error(transparent)]
    Secrets(#[from] SecretsError),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Throttles(#[from] ThrottlesError),
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
    throttles: Arc<dyn Throttles>,
    operators: Arc<dyn Operators>,
    invitation_tickets: Arc<dyn InvitationTickets>,
    catalog: Arc<dyn AccountCatalog>,
    mailer: Arc<dyn Mailer>,
    clock: Arc<dyn Clock>,
    signing_up: SigningUp,
}

impl AccountService {
    pub fn new(
        store: Arc<dyn EventStore<AccountEvent>>,
        secrets: SecretStores,
        catalog: Arc<dyn AccountCatalog>,
        mailer: Arc<dyn Mailer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let SecretStores {
            email_addresses,
            passwords,
            verifications,
            sessions,
            throttles,
            operators,
            invitation_tickets,
        } = secrets;

        Self {
            events: Arc::new(EventSourcingService::new(store, clock.clone())),
            email_addresses,
            passwords,
            verifications,
            sessions,
            throttles,
            operators,
            invitation_tickets,
            catalog,
            mailer,
            clock,
            signing_up: SigningUp::Open,
        }
    }

    pub fn signing_up(self, signing_up: SigningUp) -> Self {
        Self { signing_up, ..self }
    }

    pub async fn sign_up(
        &self,
        email: &str,
        name: &str,
        password: &str,
    ) -> Result<(), AccountServiceError> {
        if self.signing_up == SigningUp::ByInvitation {
            return Err(AccountServiceError::SigningUpByInvitationOnly);
        }
        let email = EmailAddress::new(email)?;
        let name = AccountName::new(name)?;
        let password = Password::new(password)?;
        self.counted_mail_to(&email).await?;

        let hashed = password.hashed();
        let id = AccountId::generate(self.clock.now());
        if !self.claimed(&email, &id).await? {
            return self.tell_already_signed_up(&email).await;
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

        self.mail_code(&id, &email, CodePurpose::ConfirmAddress)
            .await
    }

    pub async fn sign_up_by_invitation(
        &self,
        token: &str,
        name: &str,
        password: &str,
    ) -> Result<(), AccountServiceError> {
        let name = AccountName::new(name)?;
        let hashed = Password::new(password)?.hashed();
        let Some(ticket) = self
            .invitation_tickets
            .take_ticket(&InvitationToken::presented(token).digest())
            .await?
        else {
            return Err(AccountServiceError::InvitationInvalid);
        };
        if self.clock.now() > ticket.until {
            return Err(AccountServiceError::InvitationExpired);
        }

        let id = AccountId::generate(self.clock.now());
        if !self.claimed(&ticket.to, &id).await? {
            return Err(AccountServiceError::EmailAddressHasAccount);
        }
        if let Err(refused) = self
            .events
            .begin(
                &AggregateId::from(&id),
                AccountCommand::SignUpByInvitation {
                    email: ticket.to.clone(),
                    name,
                    invitation: ticket.invitation,
                },
                &id.acting(),
            )
            .await
        {
            self.email_addresses.release(&ticket.to, &id).await?;
            return Err(refused.into());
        }
        self.passwords.keep(&id, &hashed).await?;

        Ok(())
    }

    pub async fn keep_invitation_ticket(
        &self,
        digest: &TokenDigest,
        ticket: &InvitationTicket,
    ) -> Result<(), AccountServiceError> {
        if !self.invitation_tickets.keep_ticket(digest, ticket).await? {
            return Ok(());
        }
        let Some((id, _)) = self.whose(&ticket.to).await? else {
            return Ok(());
        };

        self.invitation_tickets
            .forget_ticket(&ticket.invitation)
            .await?;
        let _landed = self
            .events
            .execute(
                &AggregateId::from(&id),
                AccountCommand::DeclineInvitation {
                    invitation: ticket.invitation.clone(),
                    reason: DeclineReason::AddressHasAccount,
                },
                &Agent::System,
            )
            .await?;

        Ok(())
    }

    pub async fn forget_invitation_ticket(
        &self,
        invitation: &str,
    ) -> Result<(), AccountServiceError> {
        Ok(self.invitation_tickets.forget_ticket(invitation).await?)
    }

    pub async fn send_code_again(&self, email: &str) -> Result<(), AccountServiceError> {
        let email = EmailAddress::new(email)?;
        self.counted_mail_to(&email).await?;
        let Some((id, account)) = self.whose(&email).await? else {
            return Ok(());
        };
        if account.is_verified() {
            return Ok(());
        }

        self.mail_code(&id, &email, CodePurpose::ConfirmAddress)
            .await
    }

    pub async fn verify(&self, email: &str, code: &str) -> Result<(), AccountServiceError> {
        let email = EmailAddress::new(email)?;
        let Some(id) = self.email_addresses.holder(&email).await? else {
            return Err(AccountServiceError::CodeWrong);
        };
        self.checked_code(&id, CodePurpose::ConfirmAddress, code)
            .await?;

        let _landed = self
            .events
            .execute(
                &AggregateId::from(&id),
                AccountCommand::Verify,
                &id.acting(),
            )
            .await?;
        self.verifications
            .close(&id, CodePurpose::ConfirmAddress)
            .await?;

        Ok(())
    }

    pub async fn ask_to_reset_password(&self, email: &str) -> Result<(), AccountServiceError> {
        let email = EmailAddress::new(email)?;
        self.counted_mail_to(&email).await?;
        let Some((id, _account)) = self.whose(&email).await? else {
            return Ok(());
        };

        self.mail_code(&id, &email, CodePurpose::ResetPassword)
            .await
    }

    pub async fn reset_password(
        &self,
        email: &str,
        code: &str,
        password: &str,
    ) -> Result<(), AccountServiceError> {
        let email = EmailAddress::new(email)?;
        let hashed = Password::new(password)?.hashed();
        let Some((id, _account)) = self.whose(&email).await? else {
            return Err(AccountServiceError::CodeWrong);
        };
        self.checked_code(&id, CodePurpose::ResetPassword, code)
            .await?;

        let _landed = self
            .events
            .execute(
                &AggregateId::from(&id),
                AccountCommand::ResetPassword,
                &id.acting(),
            )
            .await?;
        self.passwords.keep(&id, &hashed).await?;
        self.verifications
            .close(&id, CodePurpose::ResetPassword)
            .await?;

        Ok(())
    }

    async fn checked_code(
        &self,
        id: &AccountId,
        purpose: CodePurpose,
        code: &str,
    ) -> Result<(), AccountServiceError> {
        let pending = match self
            .verifications
            .attempt(id, purpose, PendingVerification::MAX_TRIES)
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
        self.not_throttled(Guarded::SigningIn, &email).await?;

        let Some((id, account, kept)) = self.signing_in_as(&email).await? else {
            let _same_work = PasswordHash::stand_in().admits(password);
            self.count_try(Guarded::SigningIn, &email).await?;
            return Err(AccountServiceError::SignInRefused);
        };
        if !kept.admits(password) {
            self.count_try(Guarded::SigningIn, &email).await?;
            return Err(AccountServiceError::SignInRefused);
        }
        self.throttles
            .clear(Guarded::SigningIn.as_str(), email.as_str())
            .await?;
        if account.is_locked() {
            return Err(AccountServiceError::Locked);
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
        if account.is_locked() || !account.counts_session_begun(session.begun) {
            self.sessions.end(&digest).await?;
            return Ok(None);
        }

        Ok(Some(SignedIn { session, account }))
    }

    pub async fn end_session(&self, token: &SessionToken) -> Result<(), AccountServiceError> {
        Ok(self.sessions.end(&token.digest()).await?)
    }

    pub async fn draw_operator_code(&self) -> Result<Option<OperatorCode>, AccountServiceError> {
        if self.operators.any_appointed().await? {
            return Ok(None);
        }
        let code = OperatorCode::drawn();
        self.operators.keep_code(&code.digest()).await?;

        Ok(Some(code))
    }

    pub async fn become_operator(
        &self,
        id: &AccountId,
        code: &str,
    ) -> Result<(), AccountServiceError> {
        if !self.get(id).await?.state.is_verified() {
            return Err(AccountServiceError::NotVerifiedYet);
        }
        if !self
            .operators
            .take_code(&OperatorCode::presented(code).digest())
            .await?
        {
            return Err(AccountServiceError::OperatorCodeWrong);
        }

        let _landed = self
            .events
            .execute(
                &AggregateId::from(id),
                AccountCommand::BecomeOperator,
                &id.acting(),
            )
            .await?;
        self.operators.appoint(id).await?;

        Ok(())
    }

    pub async fn lock(
        &self,
        by: &AccountId,
        account: &AccountId,
        reason: &str,
    ) -> Result<(), AccountServiceError> {
        let reason = LockReason::new(reason).map_err(AccountServiceError::InvalidLockReason)?;
        self.operating(by).await?;

        let _landed = self
            .events
            .execute(
                &AggregateId::from(account),
                AccountCommand::Lock(LockKind::Operator, reason),
                &by.acting(),
            )
            .await?;

        Ok(())
    }

    pub async fn unlock(
        &self,
        by: &AccountId,
        account: &AccountId,
    ) -> Result<(), AccountServiceError> {
        self.operating(by).await?;

        let _landed = self
            .events
            .execute(
                &AggregateId::from(account),
                AccountCommand::Unlock(LockKind::Operator),
                &by.acting(),
            )
            .await?;

        Ok(())
    }

    pub async fn list(
        &self,
        by: &AccountId,
        after: Option<&AccountId>,
        at_most: usize,
    ) -> Result<Vec<AccountSummary>, AccountServiceError> {
        self.operating(by).await?;

        Ok(self.catalog.listed_after(after, at_most).await?)
    }

    pub async fn appoint_operator(
        &self,
        by: &AccountId,
        account: &AccountId,
    ) -> Result<(), AccountServiceError> {
        self.operating(by).await?;

        let _landed = self
            .events
            .execute(
                &AggregateId::from(account),
                AccountCommand::BecomeOperator,
                &by.acting(),
            )
            .await?;
        self.operators.appoint(account).await?;

        Ok(())
    }

    pub async fn dismiss_operator(
        &self,
        by: &AccountId,
        account: &AccountId,
    ) -> Result<(), AccountServiceError> {
        self.operating(by).await?;
        if self.operators.dismiss(account).await? == Dismissal::LastOne {
            return Err(AccountServiceError::LastOperator);
        }

        let _landed = self
            .events
            .execute(
                &AggregateId::from(account),
                AccountCommand::StopBeingOperator,
                &by.acting(),
            )
            .await?;

        Ok(())
    }

    async fn operating(&self, by: &AccountId) -> Result<(), AccountServiceError> {
        if self.get(by).await?.state.is_operator() {
            Ok(())
        } else {
            Err(AccountServiceError::NotAnOperator)
        }
    }

    async fn signing_in_as(
        &self,
        email: &EmailAddress,
    ) -> Result<Option<(AccountId, Account, PasswordHash)>, AccountServiceError> {
        let Some((id, account)) = self.whose(email).await? else {
            return Ok(None);
        };
        let Some(kept) = self.passwords.of(&id).await? else {
            return Ok(None);
        };

        Ok(Some((id, account, kept)))
    }

    async fn not_throttled(
        &self,
        guarded: Guarded,
        email: &EmailAddress,
    ) -> Result<(), AccountServiceError> {
        let now = self.clock.now();
        let Some(tries) = self
            .throttles
            .tries(guarded.as_str(), email.as_str())
            .await?
        else {
            return Ok(());
        };
        if tries.is_forgotten(now) {
            return Ok(self
                .throttles
                .clear(guarded.as_str(), email.as_str())
                .await?);
        }

        match tries.wait_before_next(guarded.rule(), now) {
            Some(wait) => Err(AccountServiceError::Throttled { wait }),
            None => Ok(()),
        }
    }

    async fn counted_mail_to(&self, email: &EmailAddress) -> Result<(), AccountServiceError> {
        self.not_throttled(Guarded::SendingCodes, email).await?;
        self.count_try(Guarded::SendingCodes, email).await
    }

    async fn count_try(
        &self,
        guarded: Guarded,
        email: &EmailAddress,
    ) -> Result<(), AccountServiceError> {
        self.throttles
            .count(guarded.as_str(), email.as_str(), self.clock.now())
            .await?;

        Ok(())
    }

    async fn claimed(
        &self,
        email: &EmailAddress,
        id: &AccountId,
    ) -> Result<bool, AccountServiceError> {
        let now = self.clock.now();
        match self.email_addresses.claim(email, id, now).await {
            Err(SecretsError::Taken) => {}
            claimed => {
                claimed?;
                return Ok(true);
            }
        }
        let Some(holder) = self.email_addresses.holder(email).await? else {
            return Ok(false);
        };

        match self.get(&holder).await {
            Err(AccountServiceError::Events(ServiceError::NotFound { .. })) => Ok(self
                .email_addresses
                .take_over(email, &holder, id, now - CLAIM_ABANDONED_AFTER, now)
                .await?),
            Ok(_) => Ok(false),
            Err(failed) => Err(failed),
        }
    }

    async fn whose(
        &self,
        email: &EmailAddress,
    ) -> Result<Option<(AccountId, Account)>, AccountServiceError> {
        let Some(id) = self.email_addresses.holder(email).await? else {
            return Ok(None);
        };
        let account = match self.get(&id).await {
            Ok(standing) => standing.state,
            Err(AccountServiceError::Events(ServiceError::NotFound { .. })) => return Ok(None),
            Err(failed) => return Err(failed),
        };

        Ok((account.email() == email).then_some((id, account)))
    }

    pub async fn get(&self, id: &AccountId) -> Result<Standing<Account>, AccountServiceError> {
        Ok(self.events.latest(&AggregateId::from(id)).await?)
    }

    async fn mail_code(
        &self,
        id: &AccountId,
        email: &EmailAddress,
        purpose: CodePurpose,
    ) -> Result<(), AccountServiceError> {
        let pending = PendingVerification::issued(self.clock.now());
        self.verifications.issue(id, purpose, &pending).await?;
        let code = pending.code.as_str();
        let (subject, body) = match purpose {
            CodePurpose::ConfirmAddress => (
                "Your Weaveling code",
                format!(
                    "{code} is your code to confirm this address for Weaveling. It is good for 15 minutes."
                ),
            ),
            CodePurpose::ResetPassword => (
                "Setting a new Weaveling password",
                format!(
                    "{code} is your code to set a new password for Weaveling. It is good for 15 minutes. If you did not ask for it, you can ignore this mail, and your password stays as it is."
                ),
            ),
        };

        Ok(self
            .mailer
            .send(Mail {
                to: email.to_string(),
                subject: subject.to_owned(),
                body,
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
