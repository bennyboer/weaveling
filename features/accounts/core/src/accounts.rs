use std::sync::Arc;

use acting::SessionEnd;
use clock::Clock;
use eventsourcing::EventStore;
use mailing::Mailer;
use time::OffsetDateTime;
use tokio::sync::watch;

use crate::account::AccountEvent;
use crate::catalog::{AccountCatalog, AccountSummary};
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::operator::OperatorCode;
use crate::secrets::SecretStores;
use crate::service::{AccountService, SignedIn};
use crate::session::{SessionToken, TokenDigest};
use crate::session_end;
use crate::signing_up::SigningUp;
use crate::ticket::InvitationTicket;

#[derive(Clone)]
pub struct AccountsCore {
    service: AccountService,
    catalog: Arc<dyn AccountCatalog>,
    clock: Arc<dyn Clock>,
    sessions_checked: Arc<watch::Sender<u64>>,
}

impl AccountsCore {
    pub fn new(
        store: Arc<dyn EventStore<AccountEvent>>,
        secrets: SecretStores,
        catalog: Arc<dyn AccountCatalog>,
        mailer: Arc<dyn Mailer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: AccountService::new(store, secrets, catalog.clone(), mailer, clock.clone()),
            catalog,
            clock,
            sessions_checked: Arc::new(watch::Sender::new(0)),
        }
    }

    pub fn signing_up(self, signing_up: SigningUp) -> Self {
        Self {
            service: self.service.signing_up(signing_up),
            ..self
        }
    }

    pub async fn sign_up(
        &self,
        email: &str,
        name: &str,
        password: &str,
    ) -> Result<(), AccountsError> {
        self.service.sign_up(email, name, password).await
    }

    pub async fn sign_up_to_operate(
        &self,
        email: &str,
        name: &str,
        password: &str,
        code: &str,
    ) -> Result<(), AccountsError> {
        self.service
            .sign_up_to_operate(email, name, password, code)
            .await
    }

    pub async fn operator_wanted(&self) -> Result<bool, AccountsError> {
        self.service.operator_wanted().await
    }

    pub async fn sign_up_by_invitation(
        &self,
        token: &str,
        name: &str,
        password: &str,
    ) -> Result<(SessionToken, SignedIn), AccountsError> {
        self.service
            .sign_up_by_invitation(token, name, password)
            .await
    }

    pub async fn take_up_invitation(
        &self,
        account: &AccountId,
        token: &str,
    ) -> Result<InvitationTicket, AccountsError> {
        self.service.take_up_invitation(account, token).await
    }

    pub async fn take_up_invitation_by_id(
        &self,
        account: &AccountId,
        invitation: &str,
    ) -> Result<InvitationTicket, AccountsError> {
        self.service
            .take_up_invitation_by_id(account, invitation)
            .await
    }

    pub async fn decline_invitation(
        &self,
        account: &AccountId,
        token: &str,
    ) -> Result<(), AccountsError> {
        self.service.decline_invitation(account, token).await
    }

    pub async fn decline_invitation_by_id(
        &self,
        account: &AccountId,
        invitation: &str,
    ) -> Result<(), AccountsError> {
        self.service
            .decline_invitation_by_id(account, invitation)
            .await
    }

    pub async fn keep_invitation_ticket(
        &self,
        digest: &TokenDigest,
        ticket: &InvitationTicket,
    ) -> Result<(), AccountsError> {
        self.service.keep_invitation_ticket(digest, ticket).await
    }

    pub async fn forget_invitation_ticket(&self, invitation: &str) -> Result<(), AccountsError> {
        self.service.forget_invitation_ticket(invitation).await
    }

    pub async fn drop_invitation_ticket(&self, invitation: &str) -> Result<(), AccountsError> {
        self.service.drop_invitation_ticket(invitation).await
    }

    pub async fn send_code_again(&self, email: &str) -> Result<(), AccountsError> {
        self.service.send_code_again(email).await
    }

    pub async fn verify(&self, email: &str, code: &str) -> Result<(), AccountsError> {
        self.service.verify(email, code).await
    }

    pub async fn ask_to_reset_password(&self, email: &str) -> Result<(), AccountsError> {
        self.service.ask_to_reset_password(email).await
    }

    pub async fn reset_password(
        &self,
        email: &str,
        code: &str,
        password: &str,
    ) -> Result<(), AccountsError> {
        self.service.reset_password(email, code, password).await?;
        self.check_sessions();

        Ok(())
    }

    pub async fn sign_in(&self, email: &str, password: &str) -> Result<AccountId, AccountsError> {
        self.service.sign_in(email, password).await
    }

    pub async fn begin_session(
        &self,
        email: &str,
        password: &str,
    ) -> Result<(SessionToken, SignedIn), AccountsError> {
        self.service.begin_session(email, password).await
    }

    pub async fn signed_in(&self, token: &SessionToken) -> Result<Option<SignedIn>, AccountsError> {
        self.service.signed_in(token).await
    }

    pub async fn end_session(&self, token: &SessionToken) -> Result<(), AccountsError> {
        self.service.end_session(token).await?;
        self.check_sessions();

        Ok(())
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

    fn check_sessions(&self) {
        self.sessions_checked.send_modify(|checked| *checked += 1);
    }

    pub async fn draw_operator_code(&self) -> Result<Option<OperatorCode>, AccountsError> {
        self.service.draw_operator_code().await
    }

    pub async fn become_operator(&self, id: &AccountId, code: &str) -> Result<(), AccountsError> {
        self.service.become_operator(id, code).await
    }

    pub async fn lock(
        &self,
        by: &AccountId,
        account: &AccountId,
        reason: &str,
    ) -> Result<(), AccountsError> {
        self.operating(by).await?;

        self.service.lock(by, account, reason).await?;
        self.check_sessions();

        Ok(())
    }

    pub async fn unlock(&self, by: &AccountId, account: &AccountId) -> Result<(), AccountsError> {
        self.operating(by).await?;

        self.service.unlock(by, account).await
    }

    pub async fn list(
        &self,
        by: &AccountId,
        after: Option<&AccountId>,
        at_most: usize,
    ) -> Result<Vec<AccountSummary>, AccountsError> {
        self.operating(by).await?;

        self.service.list(after, at_most).await
    }

    pub async fn appoint_operator(
        &self,
        by: &AccountId,
        account: &AccountId,
    ) -> Result<(), AccountsError> {
        self.operating(by).await?;

        self.service.appoint_operator(by, account).await
    }

    pub async fn dismiss_operator(
        &self,
        by: &AccountId,
        account: &AccountId,
    ) -> Result<(), AccountsError> {
        self.operating(by).await?;

        self.service.dismiss_operator(by, account).await
    }

    pub async fn catalogue(&self, id: &AccountId) -> Result<(), AccountsError> {
        let standing = self.service.get(id).await?;

        Ok(self
            .catalog
            .remember(&AccountSummary::of(*id, standing.version, &standing.state))
            .await?)
    }

    async fn operating(&self, by: &AccountId) -> Result<(), AccountsError> {
        if self.service.get(by).await?.state.is_operator() {
            Ok(())
        } else {
            Err(AccountsError::NotAnOperator)
        }
    }
}
