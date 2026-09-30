use std::sync::Arc;

use boards_catalog::InMemoryBoardCatalog;
use boards_core::{BoardCatalog, BoardEvent, BoardService};
use boards_messaging::{
    BoardCatalogProjector, DiscardBoardsOnProjectDeleted, PinnedPiecesProjector, UnpinOnDiscard,
};
use eventsourcing::{EventStore, InMemoryEventStore, InMemoryOutbox, Outbox};
use registry::{InMemoryRegistry, Registry};
use wiring::{Context, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<BoardEvent>>,
    pub catalog: Arc<dyn BoardCatalog>,
    pub registry: Arc<dyn Registry>,
    pub outbox: Arc<dyn Outbox>,
}

impl Ports {
    pub fn in_memory(
        publisher: Arc<dyn messaging::Publisher>,
        clock: Arc<dyn clock::Clock>,
    ) -> Self {
        let outbox = Arc::new(InMemoryOutbox::new(publisher, clock));

        Self {
            events: Arc::new(InMemoryEventStore::enqueuing_to(
                outbox.clone(),
                boards_messaging::message_for,
            )),
            catalog: Arc::new(InMemoryBoardCatalog::new()),
            registry: Arc::new(InMemoryRegistry::new()),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    pub fn postgres(
        pool: sqlx::PgPool,
        publisher: Arc<dyn messaging::Publisher>,
        clock: Arc<dyn clock::Clock>,
    ) -> Self {
        use eventsourcing::{PostgresEventStore, PostgresOutbox};

        Self {
            events: Arc::new(PostgresEventStore::new(
                pool.clone(),
                boards_store::codec(),
                boards_messaging::message_for,
            )),
            catalog: Arc::new(boards_catalog::PostgresBoardCatalog::new(pool.clone())),
            registry: Arc::new(registry::PostgresRegistry::new(pool.clone())),
            outbox: Arc::new(PostgresOutbox::new(pool, publisher, clock)),
        }
    }
}

pub fn service(ports: &Ports, context: &Context) -> BoardService {
    BoardService::new(
        ports.events.clone(),
        ports.registry.clone(),
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
    let sweep = DiscardBoardsOnProjectDeleted::new(boards.clone(), ports.catalog.clone());

    Wired::serving(boards_rest::router(boards)).listening(vec![
        Arc::new(catalogue),
        Arc::new(index),
        Arc::new(tidy),
        Arc::new(sweep),
    ])
}

pub const NAME: &str = "boards";

#[cfg(feature = "postgres")]
pub async fn lay_out(pool: &sqlx::PgPool) -> Result<(), wiring::Unprepared> {
    wiring::database::lay_out(NAME, pool, eventsourcing::migrations()).await?;
    wiring::database::lay_out(NAME, pool, boards_catalog::migrations()).await?;
    wiring::database::lay_out(NAME, pool, registry::migrations()).await
}
