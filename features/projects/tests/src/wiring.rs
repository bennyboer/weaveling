use std::sync::Arc;

use axum::Router;
use clock::Clock;
use eventsourcing::{InMemoryEventStore, InMemoryOutbox, Outbox};
use messaging::{DeliveryConsumer, InMemoryDeliveries, InProcessDispatcher, Listener};
use projects_catalog::InMemoryProjectCatalog;
use projects_core::ProjectService;
use wiring::Context;

const CATALOGUING: &str = "catalogue-project";

pub struct Wired {
    pub projects: ProjectService,
    pub routes: Router,
    pub outbox: Arc<InMemoryOutbox>,
    pub consuming: DeliveryConsumer,
    pub catalog: Arc<InMemoryProjectCatalog>,
    pub projector: Arc<dyn Listener>,
}

pub fn wired(clock: Arc<dyn Clock>) -> Wired {
    let deliveries = Arc::new(InMemoryDeliveries::new());
    let dispatcher = Arc::new(InProcessDispatcher::queueing_to(deliveries.clone()));
    let outbox = Arc::new(InMemoryOutbox::new(dispatcher.clone(), clock.clone()));
    let store = Arc::new(InMemoryEventStore::enqueuing_to(
        outbox.clone(),
        projects_messaging::message_for,
    ));
    let catalog = Arc::new(InMemoryProjectCatalog::new());
    let ports = projects_wiring::Ports {
        events: store.clone(),
        catalog: catalog.clone(),
        outbox: outbox.clone(),
    };
    let context = Context {
        clock,
        publisher: dispatcher.clone(),
    };
    let wired = projects_wiring::wire(&ports, &context);

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
        projects: projects_wiring::service(&ports, &context),
        routes: wired.routes,
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
