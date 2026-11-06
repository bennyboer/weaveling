use std::sync::Arc;

use accounts_catalog::InMemoryAccountCatalog;
use accounts_core::{
    Account, AccountError, AccountEvent, AccountId, AccountsCore, Dismissal, EmailAddress,
    EmailAddresses, InvitationTicket, InvitationTickets, Operators, SecretStores, SecretsError,
    Session, Sessions, TokenDigest,
};
use accounts_secrets::InMemorySecrets;
use accounts_wiring::Ports;
use async_trait::async_trait;
use authorizing::AllowingEverything;
use axum::Router;
use clock::FixedClock;
use eventsourcing::{
    AggregateId, EventSourcingService, EventStore, FailsAppending, InMemoryEventStore,
    RefusesAppending, ServiceError, Standing,
};
use inbox::InMemoryInbox;
use mailing::FakeMailbox;
use messaging::{DeliveryConsumer, InMemoryDeliveries, InProcessDispatcher};
use outbox::{InMemoryOutbox, Outbox};
use throttling::InMemoryThrottles;
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transaction, Transactions};
use wiring::Context;

pub const ADA: &str = "ada@example.com";
pub const HER_PASSWORD: &str = "correct horse battery";

pub struct Wired {
    pub accounts: AccountsCore,
    pub routes: Router,
    pub store: Arc<InMemoryEventStore<AccountEvent>>,
    pub secrets: Arc<InMemorySecrets>,
    pub throttles: Arc<InMemoryThrottles>,
    pub told: Arc<InMemoryOutbox>,
    pub mailbox: Arc<FakeMailbox>,
    pub clock: Arc<FixedClock>,
    pub catalog: Arc<InMemoryAccountCatalog>,
    pub consuming: DeliveryConsumer,
    pub publisher: Arc<InProcessDispatcher>,
}

impl Wired {
    pub async fn account(
        &self,
        id: &AccountId,
    ) -> Result<Standing<Account>, ServiceError<AccountError>> {
        EventSourcingService::new(self.store.clone(), self.clock.clone())
            .latest(&AggregateId::from(id))
            .await
    }
}

pub fn wired() -> Wired {
    let clock = Arc::new(FixedClock::new(clock::at(1_700_000_000)));
    let deliveries = Arc::new(InMemoryDeliveries::new());
    let dispatcher = Arc::new(InProcessDispatcher::queueing_to(deliveries.clone()));
    let told = Arc::new(InMemoryOutbox::new(dispatcher.clone(), clock.clone()));
    let store = Arc::new(InMemoryEventStore::enqueuing_to(
        told.clone(),
        accounts_messaging::message_for,
    ));
    let secrets = Arc::new(InMemorySecrets::default());
    let throttles = Arc::new(InMemoryThrottles::default());
    let mailbox = Arc::new(FakeMailbox::default());
    let catalog = Arc::new(InMemoryAccountCatalog::new());
    let ports = Ports {
        events: store.clone(),
        transactions: Arc::new(InMemoryTransactions::new()),
        secrets: SecretStores::kept_by(secrets.clone(), throttles.clone()),
        catalog: catalog.clone(),
        inbox: Arc::new(InMemoryInbox::new(clock.clone())),
        mailer: mailbox.clone(),
        outbox: told.clone(),
    };
    let context = Context {
        clock: clock.clone(),
        publisher: dispatcher.clone(),
        authorizer: Arc::new(AllowingEverything),
    };
    let wired = accounts_wiring::wire(&ports, &context);
    for listener in &wired.listeners {
        dispatcher.listen(listener.clone());
    }

    Wired {
        accounts: accounts_wiring::core(&ports, &context),
        routes: wired.routes,
        store,
        secrets,
        throttles,
        told,
        mailbox,
        consuming: DeliveryConsumer::new(dispatcher.clone(), deliveries, clock.clone()),
        publisher: dispatcher,
        clock,
        catalog,
    }
}

impl Wired {
    pub fn refusing_appends(&self) -> AccountsCore {
        self.over(Arc::new(RefusesAppending(self.store.clone())))
    }

    pub fn refusing_to_open_sessions(&self) -> AccountsCore {
        AccountsCore::new(
            self.store.clone(),
            Arc::new(InMemoryTransactions::new()),
            SecretStores {
                sessions: Arc::new(RefusesOpening(self.secrets.clone())),
                ..SecretStores::kept_by(self.secrets.clone(), self.throttles.clone())
            },
            self.catalog.clone(),
            self.mailbox.clone(),
            self.clock.clone(),
        )
    }

    pub fn losing_races(&self, times: u8) -> AccountsCore {
        self.over(Arc::new(FailsAppending::losing_races(
            self.store.clone(),
            times,
        )))
    }

    fn over(&self, store: Arc<dyn EventStore<AccountEvent>>) -> AccountsCore {
        AccountsCore::new(
            store,
            Arc::new(InMemoryTransactions::new()),
            SecretStores::kept_by(self.secrets.clone(), self.throttles.clone()),
            self.catalog.clone(),
            self.mailbox.clone(),
            self.clock.clone(),
        )
    }

    pub async fn claim(
        &self,
        email: &EmailAddress,
        account: &AccountId,
        at: OffsetDateTime,
    ) -> Result<(), SecretsError> {
        let mut transaction = begun().await;
        self.secrets
            .claim(&mut transaction, email, account, at)
            .await?;
        finished(transaction).await;

        Ok(())
    }

    pub async fn keep_ticket(
        &self,
        digest: &TokenDigest,
        ticket: &InvitationTicket,
    ) -> Result<bool, SecretsError> {
        let mut transaction = begun().await;
        let kept = self
            .secrets
            .keep_ticket(&mut transaction, digest, ticket)
            .await?;
        finished(transaction).await;

        Ok(kept)
    }

    pub async fn take_ticket(
        &self,
        digest: &TokenDigest,
    ) -> Result<Option<InvitationTicket>, SecretsError> {
        let mut transaction = begun().await;
        let taken = self.secrets.take_ticket(&mut transaction, digest).await?;
        finished(transaction).await;

        Ok(taken)
    }

    pub async fn dismiss(&self, account: &AccountId) -> Result<Dismissal, SecretsError> {
        let mut transaction = begun().await;
        let dismissal = self.secrets.dismiss(&mut transaction, account).await?;
        finished(transaction).await;

        Ok(dismissal)
    }

    pub async fn open(&self, digest: &TokenDigest, session: &Session) -> Result<(), SecretsError> {
        let mut transaction = begun().await;
        self.secrets.open(&mut transaction, digest, session).await?;
        finished(transaction).await;

        Ok(())
    }

    pub async fn forget_ticket(&self, invitation: &str) -> Result<(), SecretsError> {
        let mut transaction = begun().await;
        self.secrets
            .forget_ticket(&mut transaction, invitation)
            .await?;
        finished(transaction).await;

        Ok(())
    }

    pub async fn settle(&self) {
        loop {
            let published = self
                .told
                .deliver(128)
                .await
                .expect("delivering should succeed")
                .published;
            let taken = self.consuming.drain(128).await;

            if published == 0 && taken == 0 {
                break;
            }
        }
    }

    pub fn codes_sent_to(&self, address: &str) -> Vec<String> {
        self.mailbox
            .sent_to(address)
            .iter()
            .filter_map(|mail| {
                mail.body
                    .split_whitespace()
                    .find(|word| {
                        word.len() == 6 && word.bytes().all(|digit| digit.is_ascii_digit())
                    })
                    .map(str::to_owned)
            })
            .collect()
    }

    pub fn last_code_sent_to(&self, address: &str) -> String {
        self.codes_sent_to(address)
            .pop()
            .expect("a code should have been sent")
    }
}

async fn begun() -> Transaction {
    InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed")
}

async fn finished(transaction: Transaction) {
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
}

struct RefusesOpening(Arc<InMemorySecrets>);

#[async_trait]
impl Sessions for RefusesOpening {
    async fn open(
        &self,
        _transaction: &mut Transaction,
        _digest: &TokenDigest,
        _session: &Session,
    ) -> Result<(), SecretsError> {
        Err(SecretsError::Backend(
            "the sessions cannot be reached".into(),
        ))
    }

    async fn find(&self, digest: &TokenDigest) -> Result<Option<Session>, SecretsError> {
        self.0.find(digest).await
    }

    async fn end(
        &self,
        transaction: &mut Transaction,
        digest: &TokenDigest,
    ) -> Result<(), SecretsError> {
        self.0.end(transaction, digest).await
    }
}
