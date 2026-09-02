use std::sync::Arc;

use eventsourcing::{EventStore, InMemoryEventStore};
use pieces_catalog::InMemoryPieceCatalog;
use pieces_core::{PieceCatalog, PieceEvent, PieceService};
use pieces_messaging::{PieceCatalogProjector, Publishing};
use wiring::{Context, Wired};

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

pub fn service(ports: &Ports, context: &Context) -> PieceService {
    PieceService::new(
        ports.events.clone(),
        ports.catalog.clone(),
        Arc::new(Publishing::new(context.publisher.clone())),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let projector = PieceCatalogProjector::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    );

    Wired::serving(pieces_rest::router(service(ports, context)))
        .listening(vec![Arc::new(projector)])
}
