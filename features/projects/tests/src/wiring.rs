use std::sync::Arc;

use authorizing::{AllowingEverything, Authorizer};
use axum::Router;
use chores::Chore;
use clock::Clock;
use eventsourcing::InMemoryEventStore;
use inbox::InMemoryInbox;
use live::Topic;
use messaging::{DeliveryConsumer, InMemoryDeliveries, InProcessDispatcher, Listener};
use outbox::{InMemoryOutbox, Outbox};
use projects_catalog::InMemoryProjectCatalog;
use projects_core::ProjectsCore;
use transacting::InMemoryTransactions;
use trash::InMemoryTrash;
use wiring::Context;

const CATALOGUING: &str = "catalogue-project";
const PURGING: &str = "purge-the-projects-trash";
const ABANDONING: &str = "delete-abandoned-project";

pub struct Wired {
    pub projects: ProjectsCore,
    pub routes: Router,
    pub topics: Vec<Arc<dyn Topic>>,
    pub outbox: Arc<InMemoryOutbox>,
    pub consuming: DeliveryConsumer,
    pub catalog: Arc<InMemoryProjectCatalog>,
    pub trash: Arc<InMemoryTrash>,
    pub projector: Arc<dyn Listener>,
    pub abandoning: Arc<dyn Listener>,
    pub purging: Arc<dyn Chore>,
}

pub fn wired(clock: Arc<dyn Clock>) -> Wired {
    wired_guarded_by(clock, Arc::new(AllowingEverything))
}

pub fn wired_guarded_by(clock: Arc<dyn Clock>, authorizer: Arc<dyn Authorizer>) -> Wired {
    let deliveries = Arc::new(InMemoryDeliveries::new());
    let dispatcher = Arc::new(InProcessDispatcher::queueing_to(deliveries.clone()));
    let outbox = Arc::new(InMemoryOutbox::new(dispatcher.clone(), clock.clone()));
    let store = Arc::new(InMemoryEventStore::enqueuing_to(
        outbox.clone(),
        projects_messaging::message_for,
    ));
    let catalog = Arc::new(InMemoryProjectCatalog::new());
    let trash = Arc::new(InMemoryTrash::new());
    let ports = projects_wiring::Ports {
        events: store.clone(),
        transactions: Arc::new(InMemoryTransactions::new()),
        catalog: catalog.clone(),
        trash: trash.clone(),
        inbox: Arc::new(InMemoryInbox::new(clock.clone())),
        outbox: outbox.clone(),
    };
    let context = Context {
        clock,
        publisher: dispatcher.clone(),
        authorizer,
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
    let abandoning = wired
        .listeners
        .iter()
        .find(|listener| listener.named().as_str() == ABANDONING)
        .cloned()
        .expect("the feature should wire a listener deleting a project nobody is left in");
    let purging = wired
        .chores
        .iter()
        .find(|chore| chore.named() == PURGING)
        .cloned()
        .expect("the feature should wire a chore that purges its trash");

    Wired {
        projects: projects_wiring::core(&ports, &context),
        routes: wired
            .routes
            .merge(live::live_socket(live::Topics::of(wired.topics.clone()))),
        topics: wired.topics,
        outbox,
        consuming: DeliveryConsumer::new(dispatcher.clone(), deliveries, context.clock.clone()),
        catalog,
        trash,
        projector,
        abandoning,
        purging,
    }
}

impl Wired {
    pub async fn purged(&self) -> u64 {
        let purged = self
            .purging
            .carry_out()
            .await
            .expect("purging should succeed");
        self.settle().await;

        purged
    }

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
