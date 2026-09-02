use std::sync::Arc;

use axum::Router;
use boards_catalog::InMemoryBoardCatalog;
use boards_core::{BoardEvent, BoardService};
use clock::Clock;
use eventsourcing::InMemoryEventStore;
use messaging::{InProcessDispatcher, Listener};
use wiring::Context;

const CATALOGUING: &str = "catalogue-board";
const INDEXING: &str = "index-pinned-pieces";
const TIDYING: &str = "unpin-discarded-piece";

pub struct Wired {
    pub boards: BoardService,
    pub routes: Router,
    pub store: Arc<InMemoryEventStore<BoardEvent>>,
    pub catalog: Arc<InMemoryBoardCatalog>,
    pub projector: Arc<dyn Listener>,
    pub indexer: Arc<dyn Listener>,
    pub tidier: Arc<dyn Listener>,
}

pub fn wired(clock: Arc<dyn Clock>) -> Wired {
    let store = Arc::new(InMemoryEventStore::<BoardEvent>::new());
    let catalog = Arc::new(InMemoryBoardCatalog::new());
    let dispatcher = Arc::new(InProcessDispatcher::new());
    let ports = boards_wiring::Ports {
        events: store.clone(),
        catalog: catalog.clone(),
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
        catalog,
        projector,
        indexer,
        tidier,
    }
}
