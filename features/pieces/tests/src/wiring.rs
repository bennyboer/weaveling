use std::sync::Arc;

use clock::Clock;
use eventsourcing::InMemoryEventStore;
use messaging::InProcessDispatcher;
use pieces_catalog::InMemoryPieceCatalog;
use pieces_core::{PieceEvent, PieceService};
use pieces_messaging::{PieceCatalogProjector, Publishing};

pub struct Wired {
    pub pieces: PieceService,
    pub store: Arc<InMemoryEventStore<PieceEvent>>,
    pub catalog: Arc<InMemoryPieceCatalog>,
    pub cataloguing: Arc<PieceCatalogProjector>,
}

pub fn wired(clock: Arc<dyn Clock>) -> Wired {
    let store = Arc::new(InMemoryEventStore::<PieceEvent>::new());
    let catalog = Arc::new(InMemoryPieceCatalog::new());
    let dispatcher = Arc::new(InProcessDispatcher::new());

    let cataloguing = Arc::new(PieceCatalogProjector::new(
        store.clone(),
        catalog.clone(),
        clock.clone(),
    ));
    dispatcher.listen(cataloguing.clone());

    Wired {
        pieces: PieceService::new(
            store.clone(),
            catalog.clone(),
            Arc::new(Publishing::new(dispatcher)),
            clock,
        ),
        store,
        catalog,
        cataloguing,
    }
}
