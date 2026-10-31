use std::sync::Arc;

use accounts_catalog::InMemoryAccountCatalog;
use accounts_core::{
    Account, AccountError, AccountEvent, AccountId, AccountsCore, EmailAddress, EmailAddresses,
    InvitationTicket, InvitationTickets, SecretStores, SecretsError, TokenDigest,
};
use accounts_secrets::InMemorySecrets;
use accounts_wiring::Ports;
use async_trait::async_trait;
use authorizing::AllowingEverything;
use axum::Router;
use clock::FixedClock;
use eventsourcing::{
    AggregateId, AggregateType, EventSourcingService, EventStore, InMemoryEventStore, Recorded,
    ServiceError, Standing, StoreError, Version,
};
use mailing::FakeMailbox;
use messaging::{DeliveryConsumer, InMemoryDeliveries, InProcessDispatcher};
use outbox::{InMemoryOutbox, Outbox};
use throttling::InMemoryThrottles;
use time::{Duration, OffsetDateTime};
use transacting::{InMemoryTransactions, Transaction, Transactions};
use wiring::Context;

pub const ADA: &str = "ada@example.com";
pub const HER_PASSWORD: &str = "correct horse battery";

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

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
        EventSourcingService::new(
            self.store.clone(),
            Arc::new(InMemoryTransactions::new()),
            self.clock.clone(),
        )
        .latest(&AggregateId::from(id))
        .await
    }
}

pub fn wired() -> Wired {
    let clock = Arc::new(FixedClock::new(at(1_700_000_000)));
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
        AccountsCore::new(
            Arc::new(RefusesAppending(self.store.clone())),
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

struct RefusesAppending(Arc<InMemoryEventStore<AccountEvent>>);

#[async_trait]
impl EventStore<AccountEvent> for RefusesAppending {
    async fn append(
        &self,
        _transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        _expected: Version,
        _events: &[Recorded<AccountEvent>],
    ) -> Result<(), StoreError> {
        Err(StoreError::backend(
            aggregate,
            kind,
            "the database went away",
        ))
    }

    async fn read_from(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
    ) -> Result<Vec<Recorded<AccountEvent>>, StoreError> {
        self.0.read_from(aggregate, kind, from).await
    }

    async fn read_through(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
        through: Version,
    ) -> Result<Vec<Recorded<AccountEvent>>, StoreError> {
        self.0.read_through(aggregate, kind, from, through).await
    }

    async fn latest_snapshot(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Option<Recorded<AccountEvent>>, StoreError> {
        self.0.latest_snapshot(aggregate, kind).await
    }

    async fn snapshot_at_or_before(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        version: Version,
    ) -> Result<Option<Recorded<AccountEvent>>, StoreError> {
        self.0.snapshot_at_or_before(aggregate, kind, version).await
    }

    async fn prune_through(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        through: Version,
    ) -> Result<(), StoreError> {
        self.0
            .prune_through(transaction, aggregate, kind, through)
            .await
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<(), StoreError> {
        self.0.forget(transaction, aggregate, kind).await
    }
}
