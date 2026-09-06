use std::sync::Arc;

use axum::Router;
use clock::Clock;
use eventsourcing::InMemoryEventStore;
use messaging::{InProcessDispatcher, Listener};
use outline_catalog::InMemoryOutlineCatalog;
use outline_core::{OutlineEvent, OutlineService};
use wiring::Context;

const CATALOGUING: &str = "catalogue-outline";
const INDEXING: &str = "index-attached-pieces";
const TIDYING: &str = "detach-discarded-piece";

pub struct Wired {
    pub outlines: OutlineService,
    pub reopened: Box<dyn Fn() -> OutlineService + Send + Sync>,
    pub routes: Router,
    pub store: Arc<InMemoryEventStore<OutlineEvent>>,
    pub catalog: Arc<InMemoryOutlineCatalog>,
    pub projector: Arc<dyn Listener>,
    pub indexer: Arc<dyn Listener>,
    pub tidier: Arc<dyn Listener>,
}

pub fn wired(clock: Arc<dyn Clock>) -> Wired {
    let store = Arc::new(InMemoryEventStore::<OutlineEvent>::new());
    let catalog = Arc::new(InMemoryOutlineCatalog::new());
    let dispatcher = Arc::new(InProcessDispatcher::new());
    let ports = outline_wiring::Ports {
        events: store.clone(),
        catalog: catalog.clone(),
    };
    let context = Context {
        clock,
        publisher: dispatcher.clone(),
    };
    let wired = outline_wiring::wire(&ports, &context);
    let afresh = {
        let store = store.clone();
        let catalog = catalog.clone();
        let clock = context.clock.clone();
        let publisher = dispatcher.clone();

        move || {
            outline_wiring::service(
                &outline_wiring::Ports {
                    events: store.clone(),
                    catalog: catalog.clone(),
                },
                &Context {
                    clock: clock.clone(),
                    publisher: publisher.clone(),
                },
            )
        }
    };

    for listener in &wired.listeners {
        dispatcher.listen(listener.clone());
    }

    let named = |wanted: &str| {
        wired
            .listeners
            .iter()
            .find(|listener| listener.named().as_str() == wanted)
            .cloned()
            .unwrap_or_else(|| panic!("the feature should wire a {wanted} listener"))
    };
    let projector = named(CATALOGUING);
    let indexer = named(INDEXING);
    let tidier = named(TIDYING);

    Wired {
        outlines: outline_wiring::service(&ports, &context),
        reopened: Box::new(afresh),
        routes: wired.routes,
        store,
        catalog,
        projector,
        indexer,
        tidier,
    }
}
