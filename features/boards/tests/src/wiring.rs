use std::sync::Arc;

use axum::Router;
use boards_catalog::InMemoryBoardCatalog;
use boards_core::{BoardEvent, BoardService};
use clock::Clock;
use eventsourcing::{InMemoryEventStore, InMemoryOutbox, Outbox};
use messaging::{InProcessDispatcher, Listener};
use registry::InMemoryRegistry;
use wiring::Context;

const CATALOGUING: &str = "catalogue-board";
const INDEXING: &str = "index-pinned-pieces";
const TIDYING: &str = "unpin-discarded-piece";

pub struct Wired {
    pub boards: BoardService,
    pub routes: Router,
    pub store: Arc<InMemoryEventStore<BoardEvent>>,
    pub outbox: Arc<InMemoryOutbox>,
    pub registry: Arc<InMemoryRegistry>,
    pub catalog: Arc<InMemoryBoardCatalog>,
    pub projector: Arc<dyn Listener>,
    pub indexer: Arc<dyn Listener>,
    pub tidier: Arc<dyn Listener>,
}

pub fn wired(clock: Arc<dyn Clock>) -> Wired {
    let dispatcher = Arc::new(InProcessDispatcher::new());
    let outbox = Arc::new(InMemoryOutbox::new(dispatcher.clone(), clock.clone()));
    let store = Arc::new(InMemoryEventStore::enqueuing_to(
        outbox.clone(),
        boards_messaging::message_for,
    ));
    let registry = Arc::new(InMemoryRegistry::new());
    let catalog = Arc::new(InMemoryBoardCatalog::new());
    let ports = boards_wiring::Ports {
        events: store.clone(),
        catalog: catalog.clone(),
        registry: registry.clone(),
        outbox: outbox.clone(),
    };
    let context = Context {
        clock,
        publisher: dispatcher.clone(),
    };
    let wired = boards_wiring::wire(&ports, &context);

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
        boards: boards_wiring::service(&ports, &context),
        routes: wired.routes,
        store,
        outbox,
        registry,
        catalog,
        projector,
        indexer,
        tidier,
    }
}

impl Wired {
    pub async fn settle(&self) {
        while self
            .outbox
            .deliver(128)
            .await
            .expect("delivering should succeed")
            .published
            > 0
        {}
    }
}
