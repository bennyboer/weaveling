use std::sync::Arc;

use eventsourcing::{EventStore, InMemoryEventStore, PublishingEventStore};
use pieces_catalog::InMemoryPieceCatalog;
use pieces_core::{PieceCatalog, PieceEvent, PieceService};
use pieces_messaging::{PieceCatalogProjector, Publishing};
use wiring::{Context, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<PieceEvent>>,
    pub catalog: Arc<dyn PieceCatalog>,
}

impl Ports {
    pub fn in_memory(publisher: Arc<dyn messaging::Publisher>) -> Self {
        Self {
            events: PublishingEventStore::wrapping(
                Arc::new(InMemoryEventStore::new()),
                Arc::new(Publishing::new(publisher)),
            ),
            catalog: Arc::new(InMemoryPieceCatalog::new()),
        }
    }

    #[cfg(feature = "postgres")]
    pub fn postgres(pool: sqlx::PgPool) -> Self {
        use eventsourcing::PostgresEventStore;

        Self {
            events: Arc::new(PostgresEventStore::new(
                pool.clone(),
                pieces_store::codec(),
                pieces_messaging::message_for,
            )),
            catalog: Arc::new(pieces_catalog::PostgresPieceCatalog::new(pool)),
        }
    }
}

pub fn service(ports: &Ports, context: &Context) -> PieceService {
    PieceService::new(
        ports.events.clone(),
        ports.catalog.clone(),
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

pub const NAME: &str = "pieces";

#[cfg(feature = "postgres")]
pub async fn lay_out(pool: &sqlx::PgPool) -> Result<(), wiring::Unprepared> {
    wiring::database::lay_out(NAME, pool, eventsourcing::migrations()).await?;
    wiring::database::lay_out(NAME, pool, pieces_catalog::migrations()).await
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
