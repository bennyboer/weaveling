use std::sync::Arc;

use boards_catalog::InMemoryBoardCatalog;
use boards_core::{BoardCatalog, BoardEvent, BoardService};
use boards_messaging::{BoardCatalogProjector, Publishing};
use eventsourcing::{EventStore, InMemoryEventStore};
use wiring::{Context, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<BoardEvent>>,
    pub catalog: Arc<dyn BoardCatalog>,
}

impl Ports {
    pub fn in_memory() -> Self {
        Self {
            events: Arc::new(InMemoryEventStore::new()),
            catalog: Arc::new(InMemoryBoardCatalog::new()),
        }
    }
}

pub fn service(ports: &Ports, context: &Context) -> BoardService {
    BoardService::new(
        ports.events.clone(),
        ports.catalog.clone(),
        Arc::new(Publishing::new(context.publisher.clone())),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let projector = BoardCatalogProjector::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    );

    Wired::serving(boards_rest::router(service(ports, context)))
        .listening(vec![Arc::new(projector)])
}
