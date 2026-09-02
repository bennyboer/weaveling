use std::sync::Arc;

use boards_catalog::InMemoryBoardCatalog;
use boards_core::{BoardEvent, BoardService};
use clock::Clock;
use eventsourcing::InMemoryEventStore;
use messaging::{InProcessDispatcher, Listener};

const CATALOGUING: &str = "catalogue-board";

pub struct Wired {
    pub boards: BoardService,
    pub store: Arc<InMemoryEventStore<BoardEvent>>,
    pub catalog: Arc<InMemoryBoardCatalog>,
    pub projector: Arc<dyn Listener>,
}

pub fn wired(clock: Arc<dyn Clock>) -> Wired {
    let store = Arc::new(InMemoryEventStore::<BoardEvent>::new());
    let catalog = Arc::new(InMemoryBoardCatalog::new());
    let dispatcher = Arc::new(InProcessDispatcher::new());
    let wired = boards_wiring::wire(store.clone(), catalog.clone(), dispatcher.clone(), clock);

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
        boards: wired.boards,
        store,
        catalog,
        projector,
    }
}
