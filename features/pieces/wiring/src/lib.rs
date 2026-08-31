use std::sync::Arc;

use axum::Router;
use clock::Clock;
use eventsourcing::EventStore;
use messaging::{Listener, Publisher};
use pieces_core::{PieceCatalog, PieceEvent, PieceService};
use pieces_messaging::{PieceCatalogProjector, Publishing};

pub struct Wired {
    pub pieces: PieceService,
    pub routes: Router,
    pub listeners: Vec<Arc<dyn Listener>>,
}

pub fn wire(
    events: Arc<dyn EventStore<PieceEvent>>,
    catalog: Arc<dyn PieceCatalog>,
    publisher: Arc<dyn Publisher>,
    clock: Arc<dyn Clock>,
) -> Wired {
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
