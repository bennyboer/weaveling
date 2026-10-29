use std::sync::Arc;

use clock::Clock;
use eventsourcing::{Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing};
use mailing::{Mail, Mailer};
use time::Duration;

use crate::account::{Account, AccountCommand, AccountEvent};
use crate::catalog::{AccountCatalog, AccountSummary};
use crate::code::{CodePurpose, PendingVerification};
use crate::decline::DeclineReason;
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::lock::{LockKind, LockReason};
use crate::name::AccountName;
use crate::operator::OperatorCode;
use crate::password::{Password, PasswordHash};
use crate::secrets::{
    Attempt, Dismissal, EmailAddresses, InvitationTickets, Operators, Passwords, SecretStores,
    SecretsError, Sessions, Verifications,
};
use crate::session::{Session, SessionToken, Stage, TokenDigest};
use crate::signing_up::SigningUp;
use crate::throttle::Guarded;
use crate::ticket::{InvitationScope, InvitationTicket, InvitationToken};
use email::EmailAddress;
use throttling::Throttles;

const CLAIM_ABANDONED_AFTER: Duration = Duration::minutes(5);

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
    ) -> Result<(), AccountsError> {
        if self.signing_up == SigningUp::ByInvitation {
            return Err(AccountsError::SigningUpByInvitationOnly);
        }

        self.signed_up(email, name, password).await
    }

    pub async fn sign_up_to_operate(
        &self,
        email: &str,
        name: &str,
        password: &str,
        code: &str,
    ) -> Result<(), AccountsError> {
        if self.signing_up == SigningUp::ByInvitation {
            if !self.operator_wanted().await? {
                return Err(AccountsError::SigningUpByInvitationOnly);
            }
            if !self
                .operators
                .holds_code(&OperatorCode::presented(code).digest())
                .await?
            {
                return Err(AccountsError::OperatorCodeWrong);
            }
        }

        self.signed_up(email, name, password).await
    }

    pub async fn operator_wanted(&self) -> Result<bool, AccountsError> {
        Ok(!self.operators.any_appointed().await?)
    }

    async fn signed_up(
        &self,
        email: &str,
        name: &str,
        password: &str,
    ) -> Result<(), AccountsError> {
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
    ) -> Result<(SessionToken, SignedIn), AccountsError> {
        let name = AccountName::new(name)?;
        let hashed = Password::new(password)?.hashed();
        let digest = InvitationToken::presented(token).digest();
        let Some(found) = self.invitation_tickets.find_ticket(&digest).await? else {
            return Err(AccountsError::InvitationInvalid);
        };
        if self.whose(&found.to).await?.is_some() {
            return Err(AccountsError::EmailAddressHasAccount);
        }
        if self.signing_up == SigningUp::ByInvitation && !found.by_an_operator {
            return Err(AccountsError::InvitationNotByAnOperator);
        }
        let Some(ticket) = self.invitation_tickets.take_ticket(&digest).await? else {
            return Err(AccountsError::InvitationInvalid);
        };
        if self.clock.now() > ticket.until {
            return Err(AccountsError::InvitationExpired);
        }

        let id = AccountId::generate(self.clock.now());
        if !self.claimed(&ticket.to, &id).await? {
            return Err(AccountsError::EmailAddressHasAccount);
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
        let account = self.get(&id).await?.state;

        self.opened(id, account).await
    }

    pub async fn take_up_invitation(
        &self,
        id: &AccountId,
        token: &str,
    ) -> Result<InvitationTicket, AccountsError> {
        let digest = InvitationToken::presented(token).digest();
        let found = self.found_ticket(id, &digest).await?;

        self.taken_up(id, &digest, &found).await
    }

    pub async fn take_up_invitation_by_id(
        &self,
        id: &AccountId,
        invitation: &str,
    ) -> Result<InvitationTicket, AccountsError> {
        let (digest, found) = self.found_ticket_by_id(id, invitation).await?;

        self.taken_up(id, &digest, &found).await
    }

    pub async fn decline_invitation(
        &self,
        id: &AccountId,
        token: &str,
    ) -> Result<(), AccountsError> {
        let digest = InvitationToken::presented(token).digest();
        self.found_ticket(id, &digest).await?;

        self.turned_down(id, &digest).await
    }

    pub async fn decline_invitation_by_id(
        &self,
        id: &AccountId,
        invitation: &str,
    ) -> Result<(), AccountsError> {
        let (digest, _found) = self.found_ticket_by_id(id, invitation).await?;

        self.turned_down(id, &digest).await
    }

    async fn found_ticket_by_id(
        &self,
        id: &AccountId,
        invitation: &str,
    ) -> Result<(TokenDigest, InvitationTicket), AccountsError> {
        let Some(digest) = self.invitation_tickets.digest_of_ticket(invitation).await? else {
            return Err(AccountsError::InvitationInvalid);
        };

        match self.found_ticket(id, &digest).await {
            Ok(found) => Ok((digest, found)),
            Err(AccountsError::InvitationForAnotherAddress { .. }) => {
                Err(AccountsError::InvitationInvalid)
            }
            Err(refused) => Err(refused),
        }
    }

    async fn found_ticket(
        &self,
        id: &AccountId,
        digest: &TokenDigest,
    ) -> Result<InvitationTicket, AccountsError> {
        let Some(found) = self.invitation_tickets.find_ticket(digest).await? else {
            return Err(AccountsError::InvitationInvalid);
        };
        if found.to != *self.get(id).await?.state.email() {
            return Err(AccountsError::InvitationForAnotherAddress { to: found.to });
        }
        if self.clock.now() > found.until {
            return Err(AccountsError::InvitationExpired);
        }

        Ok(found)
    }

    async fn claimed_ticket(
        &self,
        digest: &TokenDigest,
    ) -> Result<InvitationTicket, AccountsError> {
        self.invitation_tickets
            .take_ticket(digest)
            .await?
            .ok_or(AccountsError::InvitationInvalid)
    }

    async fn taken_up(
        &self,
        id: &AccountId,
        digest: &TokenDigest,
        found: &InvitationTicket,
    ) -> Result<InvitationTicket, AccountsError> {
        if found.scope == InvitationScope::Instance {
            return Err(AccountsError::InvitationInvalid);
        }
        let ticket = self.claimed_ticket(digest).await?;

        let _landed = self
            .events
            .execute(
                &AggregateId::from(id),
                AccountCommand::TakeUpInvitation {
                    invitation: ticket.invitation.clone(),
                },
                &id.acting(),
            )
            .await?;

        Ok(ticket)
    }

    async fn turned_down(&self, id: &AccountId, digest: &TokenDigest) -> Result<(), AccountsError> {
        let ticket = self.claimed_ticket(digest).await?;

        let _landed = self
            .events
            .execute(
                &AggregateId::from(id),
                AccountCommand::DeclineInvitation {
                    invitation: ticket.invitation,
                    reason: DeclineReason::TurnedDown,
                },
                &id.acting(),
            )
            .await?;

        Ok(())
    }

    pub async fn keep_invitation_ticket(
        &self,
        digest: &TokenDigest,
        ticket: &InvitationTicket,
    ) -> Result<(), AccountsError> {
        if !self.invitation_tickets.keep_ticket(digest, ticket).await?
            || ticket.scope != InvitationScope::Instance
        {
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

    pub async fn forget_invitation_ticket(&self, invitation: &str) -> Result<(), AccountsError> {
        Ok(self.invitation_tickets.forget_ticket(invitation).await?)
    }

    pub async fn drop_invitation_ticket(&self, invitation: &str) -> Result<(), AccountsError> {
        Ok(self.invitation_tickets.drop_ticket(invitation).await?)
    }

    pub async fn send_code_again(&self, email: &str) -> Result<(), AccountsError> {
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

    pub async fn verify(&self, email: &str, code: &str) -> Result<(), AccountsError> {
        let email = EmailAddress::new(email)?;
        let Some(id) = self.email_addresses.holder(&email).await? else {
            return Err(AccountsError::CodeWrong);
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

    pub async fn ask_to_reset_password(&self, email: &str) -> Result<(), AccountsError> {
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
    ) -> Result<(), AccountsError> {
        let email = EmailAddress::new(email)?;
        let hashed = Password::new(password)?.hashed();
        let Some((id, _account)) = self.whose(&email).await? else {
            return Err(AccountsError::CodeWrong);
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
    ) -> Result<(), AccountsError> {
        let pending = match self
            .verifications
            .attempt(id, purpose, PendingVerification::MAX_TRIES)
            .await?
        {
            Attempt::Nothing => return Err(AccountsError::CodeWrong),
            Attempt::Exhausted => return Err(AccountsError::CodeExhausted),
            Attempt::Counted(pending) => pending,
        };

        if self.clock.now() > pending.until {
            return Err(AccountsError::CodeExpired);
        }
        if !pending.code.is(code) {
            return Err(AccountsError::CodeWrong);
        }

        Ok(())
    }

    pub async fn sign_in(&self, email: &str, password: &str) -> Result<AccountId, AccountsError> {
        Ok(self.signed_in_as(email, password).await?.0)
    }

    async fn signed_in_as(
        &self,
        email: &str,
        password: &str,
    ) -> Result<(AccountId, Account), AccountsError> {
        let email = EmailAddress::new(email)?;
        self.not_throttled(Guarded::SigningIn, &email).await?;

        let Some((id, account, kept)) = self.signing_in_as(&email).await? else {
            let _same_work = PasswordHash::stand_in().admits(password);
            self.count_try(Guarded::SigningIn, &email).await?;
            return Err(AccountsError::SignInRefused);
        };
        if !kept.admits(password) {
            self.count_try(Guarded::SigningIn, &email).await?;
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

        Ok((id, account))
    }

    pub async fn begin_session(
        &self,
        email: &str,
        password: &str,
    ) -> Result<(SessionToken, SignedIn), AccountsError> {
        let (id, account) = self.signed_in_as(email, password).await?;

        self.opened(id, account).await
    }

    async fn opened(
        &self,
        id: AccountId,
        account: Account,
    ) -> Result<(SessionToken, SignedIn), AccountsError> {
        let token = SessionToken::drawn();
        // TODO second factors: a session owes one, Stage::SecondFactorOwed, where the account has one
        let session = Session::begun(id, Stage::SignedIn, self.clock.now());
        self.sessions.open(&token.digest(), &session).await?;

        Ok((token, SignedIn { session, account }))
    }

    pub async fn signed_in(&self, token: &SessionToken) -> Result<Option<SignedIn>, AccountsError> {
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

    pub async fn end_session(&self, token: &SessionToken) -> Result<(), AccountsError> {
        Ok(self.sessions.end(&token.digest()).await?)
    }

    pub async fn draw_operator_code(&self) -> Result<Option<OperatorCode>, AccountsError> {
        if self.operators.any_appointed().await? {
            return Ok(None);
        }
        let code = OperatorCode::drawn();
        self.operators.keep_code(&code.digest()).await?;

        Ok(Some(code))
    }

    pub async fn become_operator(&self, id: &AccountId, code: &str) -> Result<(), AccountsError> {
        if !self.get(id).await?.state.is_verified() {
            return Err(AccountsError::NotVerifiedYet);
        }
        if !self
            .operators
            .take_code(&OperatorCode::presented(code).digest())
            .await?
        {
            return Err(AccountsError::OperatorCodeWrong);
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
    ) -> Result<(), AccountsError> {
        let reason = LockReason::new(reason).map_err(AccountsError::InvalidLockReason)?;

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

    pub async fn unlock(&self, by: &AccountId, account: &AccountId) -> Result<(), AccountsError> {
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
        after: Option<&AccountId>,
        at_most: usize,
    ) -> Result<Vec<AccountSummary>, AccountsError> {
        Ok(self.catalog.listed_after(after, at_most).await?)
    }

    pub async fn appoint_operator(
        &self,
        by: &AccountId,
        account: &AccountId,
    ) -> Result<(), AccountsError> {
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
    ) -> Result<(), AccountsError> {
        if self.operators.dismiss(account).await? == Dismissal::LastOne {
            return Err(AccountsError::LastOperator);
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

    async fn not_throttled(
        &self,
        guarded: Guarded,
        email: &EmailAddress,
    ) -> Result<(), AccountsError> {
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
            Some(wait) => Err(AccountsError::Throttled { wait }),
            None => Ok(()),
        }
    }

    async fn counted_mail_to(&self, email: &EmailAddress) -> Result<(), AccountsError> {
        self.not_throttled(Guarded::SendingCodes, email).await?;
        self.count_try(Guarded::SendingCodes, email).await
    }

    async fn count_try(&self, guarded: Guarded, email: &EmailAddress) -> Result<(), AccountsError> {
        self.throttles
            .count(guarded.as_str(), email.as_str(), self.clock.now())
            .await?;

        Ok(())
    }

    async fn claimed(&self, email: &EmailAddress, id: &AccountId) -> Result<bool, AccountsError> {
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
            Err(AccountsError::Events(ServiceError::NotFound { .. })) => Ok(self
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
    ) -> Result<Option<(AccountId, Account)>, AccountsError> {
        let Some(id) = self.email_addresses.holder(email).await? else {
            return Ok(None);
        };
        let account = match self.get(&id).await {
            Ok(standing) => standing.state,
            Err(AccountsError::Events(ServiceError::NotFound { .. })) => return Ok(None),
            Err(failed) => return Err(failed),
        };

        Ok((account.email() == email).then_some((id, account)))
    }

    pub async fn get(&self, id: &AccountId) -> Result<Standing<Account>, AccountsError> {
        Ok(self.events.latest(&AggregateId::from(id)).await?)
    }

    async fn mail_code(
        &self,
        id: &AccountId,
        email: &EmailAddress,
        purpose: CodePurpose,
    ) -> Result<(), AccountsError> {
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

    async fn tell_already_signed_up(&self, email: &EmailAddress) -> Result<(), AccountsError> {
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
