use std::sync::Arc;

use axum::Router;
use boards_catalog::InMemoryBoardCatalog;
use boards_core::{BoardCatalog, BoardEvent, BoardService};
use boards_messaging::{BoardCatalogProjector, Publishing};
use clock::Clock;
use eventsourcing::{EventStore, InMemoryEventStore};
use messaging::{Listener, Publisher};

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

pub struct Wired {
    pub boards: BoardService,
    pub routes: Router,
    pub listeners: Vec<Arc<dyn Listener>>,
}

pub fn wire(ports: Ports, publisher: Arc<dyn Publisher>, clock: Arc<dyn Clock>) -> Wired {
    let Ports { events, catalog } = ports;
    let boards = BoardService::new(
        events.clone(),
        catalog.clone(),
        Arc::new(Publishing::new(publisher)),
        clock.clone(),
    );
    let projector = BoardCatalogProjector::new(events, catalog, clock);

    Wired {
        routes: boards_rest::router(boards.clone()),
        boards,
        listeners: vec![Arc::new(projector)],
    }
}
