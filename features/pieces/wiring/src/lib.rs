use std::sync::Arc;

use eventsourcing::{EventStore, InMemoryEventStore, InMemoryOutbox, Outbox};
use pieces_catalog::InMemoryPieceCatalog;
use pieces_core::{PieceCatalog, PieceEvent, PieceService};
use pieces_messaging::{DiscardOnProjectDeleted, PieceCatalogProjector};
use wiring::{Context, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<PieceEvent>>,
    pub catalog: Arc<dyn PieceCatalog>,
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
                pieces_messaging::message_for,
            )),
            catalog: Arc::new(InMemoryPieceCatalog::new()),
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
                pieces_store::codec(),
                pieces_messaging::message_for,
            )),
            catalog: Arc::new(pieces_catalog::PostgresPieceCatalog::new(pool.clone())),
            outbox: Arc::new(PostgresOutbox::new(pool, publisher, clock)),
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
    let pieces = service(ports, context);
    let projector = PieceCatalogProjector::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    );
    let sweep = DiscardOnProjectDeleted::new(
        pieces.clone(),
        ports.catalog.clone(),
        context.publisher.clone(),
        context.clock.clone(),
    );

    Wired::serving(pieces_rest::router(pieces))
        .listening(vec![Arc::new(projector), Arc::new(sweep)])
}

pub const NAME: &str = "pieces";

#[cfg(feature = "postgres")]
pub async fn lay_out(pool: &sqlx::PgPool) -> Result<(), wiring::Unprepared> {
    wiring::database::lay_out(NAME, pool, eventsourcing::migrations()).await?;
    wiring::database::lay_out(NAME, pool, pieces_catalog::migrations()).await
}
