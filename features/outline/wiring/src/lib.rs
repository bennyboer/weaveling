use std::sync::Arc;

use eventsourcing::{EventStore, InMemoryEventStore, PublishingEventStore};
use outline_catalog::InMemoryOutlineCatalog;
use outline_core::{OutlineCatalog, OutlineEvent, OutlineService};
use outline_messaging::{
    AttachedPiecesProjector, DetachOnDiscard, OutlineCatalogProjector, Publishing,
};
use wiring::{Context, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<OutlineEvent>>,
    pub catalog: Arc<dyn OutlineCatalog>,
}

impl Ports {
    pub fn in_memory(publisher: Arc<dyn messaging::Publisher>) -> Self {
        Self {
            events: PublishingEventStore::wrapping(
                Arc::new(InMemoryEventStore::new()),
                Arc::new(Publishing::new(publisher)),
            ),
            catalog: Arc::new(InMemoryOutlineCatalog::new()),
        }
    }

    #[cfg(feature = "postgres")]
    pub fn postgres(pool: sqlx::PgPool) -> Self {
        use eventsourcing::PostgresEventStore;

        Self {
            events: Arc::new(PostgresEventStore::new(
                pool.clone(),
                outline_store::codec(),
                outline_messaging::message_for,
            )),
            catalog: Arc::new(outline_catalog::PostgresOutlineCatalog::new(pool)),
        }
    }
}

pub fn service(ports: &Ports, context: &Context) -> OutlineService {
    OutlineService::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let outlines = service(ports, context);
    let catalogue = OutlineCatalogProjector::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    );
    let index = AttachedPiecesProjector::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    );
    let tidy = DetachOnDiscard::new(outlines.clone(), ports.catalog.clone());

    Wired::serving(outline_rest::router(outlines)).listening(vec![
        Arc::new(catalogue),
        Arc::new(index),
        Arc::new(tidy),
    ])
}

pub const NAME: &str = "outline";

#[cfg(feature = "postgres")]
pub async fn lay_out(pool: &sqlx::PgPool) -> Result<(), wiring::Unprepared> {
    wiring::database::lay_out(NAME, pool, eventsourcing::migrations()).await?;
    wiring::database::lay_out(NAME, pool, outline_catalog::migrations()).await
}
