use std::sync::Arc;

use axum::Router;
use clock::Clock;
use eventsourcing::{EventStore, InMemoryEventStore};
use messaging::{Listener, Publisher};
use pieces_catalog::InMemoryPieceCatalog;
use pieces_core::{PieceCatalog, PieceEvent, PieceService};
use pieces_messaging::{PieceCatalogProjector, Publishing};

pub struct Ports {
    pub events: Arc<dyn EventStore<PieceEvent>>,
    pub catalog: Arc<dyn PieceCatalog>,
}

impl Ports {
    pub fn in_memory() -> Self {
        Self {
            events: Arc::new(InMemoryEventStore::new()),
            catalog: Arc::new(InMemoryPieceCatalog::new()),
        }
    }
}

pub struct Wired {
    pub pieces: PieceService,
    pub routes: Router,
    pub listeners: Vec<Arc<dyn Listener>>,
}

pub fn wire(ports: Ports, publisher: Arc<dyn Publisher>, clock: Arc<dyn Clock>) -> Wired {
    let Ports { events, catalog } = ports;
    let pieces = PieceService::new(
        events.clone(),
        catalog.clone(),
        Arc::new(Publishing::new(publisher)),
        clock.clone(),
    );
    let projector = PieceCatalogProjector::new(events, catalog, clock);

    Wired {
        routes: pieces_rest::router(pieces.clone()),
        listeners: vec![Arc::new(projector)],
        pieces,
    }
}
