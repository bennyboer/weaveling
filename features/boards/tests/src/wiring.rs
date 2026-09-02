use std::sync::Arc;

use axum::Router;
use boards_catalog::InMemoryBoardCatalog;
use boards_core::{BoardEvent, BoardService};
use clock::Clock;
use eventsourcing::InMemoryEventStore;
use messaging::{InProcessDispatcher, Listener};
use wiring::Context;

const CATALOGUING: &str = "catalogue-board";

pub struct Wired {
    pub boards: BoardService,
    pub routes: Router,
    pub store: Arc<InMemoryEventStore<BoardEvent>>,
    pub catalog: Arc<InMemoryBoardCatalog>,
    pub projector: Arc<dyn Listener>,
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

    let projector = wired
        .listeners
        .iter()
        .find(|listener| listener.named().as_str() == CATALOGUING)
        .cloned()
        .expect("the feature should wire a catalog projector");

    Wired {
        boards: boards_wiring::service(&ports, &context),
        routes: wired.routes,
        store,
        catalog,
        projector,
    }
}
