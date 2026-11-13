use std::sync::Arc;

use access::{AccessCatalog, InMemoryAccess, KeptAccess};
use axum::Router;
use clock::Clock;
use eventsourcing::InMemoryEventStore;
use ideas_catalog::InMemoryIdeaCatalog;
use ideas_core::{IdeaEvent, IdeasCore};
use inbox::InMemoryInbox;
use messaging::{DeliveryConsumer, InMemoryDeliveries, InProcessDispatcher, Listener};
use outbox::{InMemoryOutbox, Outbox};
use transacting::InMemoryTransactions;
use wiring::Context;

const CATALOGUING: &str = "catalogue-idea";

pub struct Wired {
    pub ideas: IdeasCore,
    pub routes: Router,
    pub store: Arc<InMemoryEventStore<IdeaEvent>>,
    pub outbox: Arc<InMemoryOutbox>,
    pub consuming: DeliveryConsumer,
    pub catalog: Arc<InMemoryIdeaCatalog>,
    pub projector: Arc<dyn Listener>,
}

/// Ideas without accounts, where everyone may do everything.
pub fn wired(clock: Arc<dyn Clock>) -> Wired {
    wired_keeping(clock, Arc::new(InMemoryAccess::new()), false)
}

/// Ideas with accounts, where who may do what is stored in the ideas' own access records.
pub fn wired_with_accounts(clock: Arc<dyn Clock>) -> Wired {
    wired_keeping(clock, Arc::new(InMemoryAccess::new()), true)
}

pub fn wired_keeping(
    clock: Arc<dyn Clock>,
    access: Arc<dyn AccessCatalog>,
    with_accounts: bool,
) -> Wired {
    let deliveries = Arc::new(InMemoryDeliveries::new());
    let dispatcher = Arc::new(InProcessDispatcher::queueing_to(deliveries.clone()));
    let outbox = Arc::new(InMemoryOutbox::new(dispatcher.clone(), clock.clone()));
    let store = Arc::new(InMemoryEventStore::enqueuing_to(
        outbox.clone(),
        ideas_messaging::message_for,
    ));
    let catalog = Arc::new(InMemoryIdeaCatalog::new());
    let ports = ideas_wiring::Ports {
        events: store.clone(),
        transactions: Arc::new(InMemoryTransactions::new()),
        catalog: catalog.clone(),
        access: KeptAccess::new(access),
        inbox: Arc::new(InMemoryInbox::new(clock.clone())),
        outbox: outbox.clone(),
    };
    let context = Context {
        with_accounts,
        clock,
        publisher: dispatcher.clone(),
    };
    let wired = ideas_wiring::wire(&ports, &context);

    for listener in &wired.listeners {
        dispatcher.listen(listener.clone());
    }

    let projector = wired
        .listeners
        .iter()
        .find(|listener| listener.named().as_str() == CATALOGUING)
        .cloned()
        .expect("the feature should wire a catalog projector");

    Wired {
        ideas: ideas_wiring::core(&ports, &context),
        routes: wired
            .routes
            .merge(live::live_socket(live::Topics::of(wired.topics))),
        store,
        outbox,
        consuming: DeliveryConsumer::new(dispatcher.clone(), deliveries, context.clock.clone()),
        catalog,
        projector,
    }
}

impl Wired {
    pub async fn settle(&self) {
        loop {
            let published = self
                .outbox
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
}
