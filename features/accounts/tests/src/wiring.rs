use std::sync::Arc;

use accounts_catalog::InMemoryAccountCatalog;
use accounts_core::{Account, AccountError, AccountEvent, AccountId, AccountsCore, SecretStores};
use accounts_secrets::InMemorySecrets;
use accounts_wiring::Ports;
use authorizing::AllowingEverything;
use axum::Router;
use clock::FixedClock;
use eventsourcing::{
    AggregateId, EventSourcingService, InMemoryEventStore, ServiceError, Standing,
};
use mailing::FakeMailbox;
use messaging::{DeliveryConsumer, InMemoryDeliveries, InProcessDispatcher};
use outbox::{InMemoryOutbox, Outbox};
use throttling::InMemoryThrottles;
use time::{Duration, OffsetDateTime};
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
        EventSourcingService::new(self.store.clone(), self.clock.clone())
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
