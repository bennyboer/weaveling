use std::sync::Arc;

use eventsourcing::{EventStore, InMemoryEventStore, InMemoryOutbox, Outbox};
use outline_catalog::InMemoryOutlineCatalog;
use outline_core::{OutlineCatalog, OutlineEvent, OutlineService};
use outline_messaging::{
    AttachedPassagesProjector, DiscardOutlinesOnProjectDeleted, OutlineCatalogProjector,
};
use registry::{InMemoryRegistry, Registry};
use wiring::{Context, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<OutlineEvent>>,
    pub catalog: Arc<dyn OutlineCatalog>,
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
                outline_messaging::message_for,
            )),
            catalog: Arc::new(InMemoryOutlineCatalog::new()),
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
                outline_store::codec(),
                outline_messaging::message_for,
            )),
            catalog: Arc::new(outline_catalog::PostgresOutlineCatalog::new(pool.clone())),
            registry: Arc::new(registry::PostgresRegistry::new(pool.clone())),
            outbox: Arc::new(PostgresOutbox::new(pool, publisher, clock)),
        }
    }
}

pub fn service(ports: &Ports, context: &Context) -> OutlineService {
    OutlineService::new(
        ports.events.clone(),
        ports.registry.clone(),
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
    let index = AttachedPassagesProjector::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    );
    let sweep = DiscardOutlinesOnProjectDeleted::new(outlines.clone(), ports.catalog.clone());

    Wired::serving(outline_rest::router(outlines)).listening(vec![
        Arc::new(catalogue),
        Arc::new(index),
        Arc::new(sweep),
    ])
}

pub const NAME: &str = "outline";

#[cfg(feature = "postgres")]
pub async fn lay_out(pool: &sqlx::PgPool) -> Result<(), wiring::Unprepared> {
    wiring::database::lay_out(NAME, pool, eventsourcing::migrations()).await?;
    wiring::database::lay_out(NAME, pool, outline_catalog::migrations()).await?;
    wiring::database::lay_out(NAME, pool, registry::migrations()).await
}
