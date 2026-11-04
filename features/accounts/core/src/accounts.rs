mod codes;
mod invitations;
mod operating;
mod sessions;
mod sign_ups;

use std::sync::Arc;

use clock::Clock;
use eventsourcing::{Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing};
use mailing::Mailer;
use throttling::{Admission, Throttles};
use time::Duration;
use tokio::sync::watch;
use transacting::{Transaction, Transactions};

use crate::account::{Account, AccountCommand, AccountEvent};
use crate::catalog::{AccountCatalog, AccountSummary};
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::secrets::{
    EmailAddresses, InvitationTickets, Operators, Passwords, SecretStores, SecretsError, Sessions,
    Verifications,
};
use crate::signing_up::SigningUp;
use crate::throttle::Guarded;
use email::EmailAddress;

pub use sessions::SignedIn;

const CLAIM_ABANDONED_AFTER: Duration = Duration::minutes(5);

#[derive(Clone)]
pub struct AccountsCore {
    events: Arc<EventSourcingService<Account>>,
    transactions: Arc<dyn Transactions>,
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
    sessions_checked: Arc<watch::Sender<u64>>,
}

impl AccountsCore {
    pub fn new(
        store: Arc<dyn EventStore<AccountEvent>>,
        transactions: Arc<dyn Transactions>,
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
            transactions,
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
            sessions_checked: Arc::new(watch::Sender::new(0)),
        }
    }

    pub fn signing_up(self, signing_up: SigningUp) -> Self {
        Self { signing_up, ..self }
    }

    pub async fn catalogue(
        &self,
        within: Option<&mut Transaction>,
        id: &AccountId,
    ) -> Result<(), AccountsError> {
        let standing = self.get(id).await?;

        let mut transaction = self.transactions.join(within).await?;
        self.catalog
            .remember(
                &mut transaction,
                &AccountSummary::of(*id, standing.version, &standing.state),
            )
            .await?;
        Ok(transaction.finish().await?)
    }

    pub(super) async fn commanded_in(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        command: AccountCommand,
        agent: &Agent,
    ) -> Result<(), AccountsError> {
        self.events
            .execute(transaction, &AggregateId::from(account), command, agent)
            .await?;

        Ok(())
    }

    pub(super) async fn admitted(
        &self,
        guarded: Guarded,
        email: &EmailAddress,
    ) -> Result<(), AccountsError> {
        match self
            .throttles
            .admit(
                guarded.as_str(),
                email.as_str(),
                guarded.rule(),
                self.clock.now(),
            )
            .await?
        {
            Admission::Admitted => Ok(()),
            Admission::Wait(wait) => Err(AccountsError::Throttled { wait }),
        }
    }

    pub(super) async fn counted_mail_to(&self, email: &EmailAddress) -> Result<(), AccountsError> {
        self.admitted(Guarded::SendingCodes, email).await
    }

    pub(super) async fn claimed(
        &self,
        transaction: &mut Transaction,
        email: &EmailAddress,
        id: &AccountId,
    ) -> Result<bool, AccountsError> {
        let now = self.clock.now();
        match self
            .email_addresses
            .claim(transaction, email, id, now)
            .await
        {
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
                .take_over(
                    transaction,
                    email,
                    &holder,
                    id,
                    now - CLAIM_ABANDONED_AFTER,
                    now,
                )
                .await?),
            Ok(_) => Ok(false),
            Err(failed) => Err(failed),
        }
    }

    pub(super) async fn whose(
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

    pub(super) async fn get(&self, id: &AccountId) -> Result<Standing<Account>, AccountsError> {
        Ok(self.events.latest(&AggregateId::from(id)).await?)
    }
}
