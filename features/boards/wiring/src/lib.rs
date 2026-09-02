use std::sync::Arc;

use boards_catalog::InMemoryBoardCatalog;
use boards_core::{BoardCatalog, BoardEvent, BoardService};
use boards_messaging::{BoardCatalogProjector, PinnedPiecesProjector, Publishing, UnpinOnDiscard};
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
    let boards = service(ports, context);
    let catalogue = BoardCatalogProjector::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    );
    let index = PinnedPiecesProjector::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    );
    let tidy = UnpinOnDiscard::new(boards.clone(), ports.catalog.clone());

    Wired::serving(boards_rest::router(boards)).listening(vec![
        Arc::new(catalogue),
        Arc::new(index),
        Arc::new(tidy),
    ])
}
