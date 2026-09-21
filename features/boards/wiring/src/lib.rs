use std::sync::Arc;

use boards_catalog::InMemoryBoardCatalog;
use boards_core::{BoardCatalog, BoardEvent, BoardService};
use boards_messaging::{BoardCatalogProjector, PinnedPiecesProjector, Publishing, UnpinOnDiscard};
use eventsourcing::{EventStore, InMemoryEventStore, PublishingEventStore};
use wiring::{Context, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<BoardEvent>>,
    pub catalog: Arc<dyn BoardCatalog>,
}

impl Ports {
    pub fn in_memory(publisher: Arc<dyn messaging::Publisher>) -> Self {
        Self {
            events: PublishingEventStore::wrapping(
                Arc::new(InMemoryEventStore::new()),
                Arc::new(Publishing::new(publisher)),
            ),
            catalog: Arc::new(InMemoryBoardCatalog::new()),
        }
    }

    #[cfg(feature = "postgres")]
    pub fn postgres(pool: sqlx::PgPool) -> Self {
        use eventsourcing::PostgresEventStore;

        Self {
            events: Arc::new(PostgresEventStore::new(
                pool.clone(),
                boards_store::codec(),
                boards_messaging::message_for,
            )),
            catalog: Arc::new(boards_catalog::PostgresBoardCatalog::new(pool)),
        }
    }
}

pub fn service(ports: &Ports, context: &Context) -> BoardService {
    BoardService::new(
        ports.events.clone(),
        ports.catalog.clone(),
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

pub const NAME: &str = "boards";

#[cfg(feature = "postgres")]
pub async fn lay_out(pool: &sqlx::PgPool) -> Result<(), wiring::Unprepared> {
    wiring::database::lay_out(NAME, pool, eventsourcing::migrations()).await?;
    wiring::database::lay_out(NAME, pool, boards_catalog::migrations()).await
}

#[cfg(feature = "postgres")]
pub fn outbox(
    pool: &sqlx::PgPool,
    publisher: Arc<dyn messaging::Publisher>,
    clock: Arc<dyn clock::Clock>,
) -> Option<eventsourcing::PostgresOutbox> {
    Some(eventsourcing::PostgresOutbox::new(
        pool.clone(),
        publisher,
        clock,
    ))
}
