use std::sync::Arc;

use boards_core::{BoardCatalog, BoardEvent, BoardService};
use boards_messaging::{BoardCatalogProjector, Publishing};
use clock::Clock;
use eventsourcing::EventStore;
use messaging::{Listener, Publisher};

pub struct Wired {
    pub boards: BoardService,
    pub listeners: Vec<Arc<dyn Listener>>,
}

pub fn wire(
    events: Arc<dyn EventStore<BoardEvent>>,
    catalog: Arc<dyn BoardCatalog>,
    publisher: Arc<dyn Publisher>,
    clock: Arc<dyn Clock>,
) -> Wired {
    let boards = BoardService::new(
        events.clone(),
        catalog.clone(),
        Arc::new(Publishing::new(publisher)),
        clock.clone(),
    );
    let projector = BoardCatalogProjector::new(events, catalog, clock);

    Wired {
        boards,
        listeners: vec![Arc::new(projector)],
    }
}
