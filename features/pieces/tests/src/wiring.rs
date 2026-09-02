use std::sync::Arc;

use axum::Router;
use clock::Clock;
use eventsourcing::InMemoryEventStore;
use messaging::{InProcessDispatcher, Listener};
use pieces_catalog::InMemoryPieceCatalog;
use pieces_core::{PieceEvent, PieceService};
use wiring::Context;

const CATALOGUING: &str = "catalogue-piece";

pub struct Wired {
    pub pieces: PieceService,
    pub routes: Router,
    pub store: Arc<InMemoryEventStore<PieceEvent>>,
    pub catalog: Arc<InMemoryPieceCatalog>,
    pub projector: Arc<dyn Listener>,
}

pub fn wired(clock: Arc<dyn Clock>) -> Wired {
    let store = Arc::new(InMemoryEventStore::<PieceEvent>::new());
    let catalog = Arc::new(InMemoryPieceCatalog::new());
    let dispatcher = Arc::new(InProcessDispatcher::new());
    let ports = pieces_wiring::Ports {
        events: store.clone(),
        catalog: catalog.clone(),
    };
    let context = Context {
        clock,
        publisher: dispatcher.clone(),
    };
    let wired = pieces_wiring::wire(&ports, &context);

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
        pieces: pieces_wiring::service(&ports, &context),
        routes: wired.routes,
        store,
        catalog,
        projector,
    }
}
