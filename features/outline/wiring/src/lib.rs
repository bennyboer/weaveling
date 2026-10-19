use std::sync::Arc;

use eventsourcing::EventStore;
use live::{BroadcastChangesListener, Broadcaster};
use outbox::Outbox;
use outline_catalog::InMemoryOutlineCatalog;
use outline_contract::EVERY_OUTLINE;
use outline_core::{OutlineCatalog, OutlineEvent, OutlineService};
use outline_messaging::{
    CatalogueOutlinesListener, DetachOnDiscardListener, DetachOnSceneDeletedListener,
    DiscardOutlinesOnProjectDeletedListener, IndexAttachmentsListener,
};
use registry::{InMemoryRegistry, Registry};
use wiring::{Context, EventSourced, Feature, Wired};

pub const LIVE: &str = "broadcast-outline-changes";

pub struct Ports {
    pub events: Arc<dyn EventStore<OutlineEvent>>,
    pub catalog: Arc<dyn OutlineCatalog>,
    pub registry: Arc<dyn Registry>,
    pub outbox: Arc<dyn Outbox>,
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
    let catalogue = CatalogueOutlinesListener::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    );
    let index = IndexAttachmentsListener::new(
        ports.events.clone(),
        ports.catalog.clone(),
        outlines.clone(),
        context.clock.clone(),
    );
    let detach_discarded_ideas =
        DetachOnDiscardListener::new(outlines.clone(), ports.catalog.clone());
    let detach_deleted_scenes =
        DetachOnSceneDeletedListener::new(outlines.clone(), ports.catalog.clone());
    let sweep = DiscardOutlinesOnProjectDeletedListener::new(outlines.clone());
    let broadcaster = Broadcaster::new();
    let broadcast = BroadcastChangesListener::new(LIVE, EVERY_OUTLINE, broadcaster.clone());

    Wired::serving(outline_rest::router(outlines).merge(outline_rest::live(broadcaster))).listening(
        vec![
            Arc::new(broadcast),
            Arc::new(catalogue),
            Arc::new(index),
            Arc::new(detach_discarded_ideas),
            Arc::new(detach_deleted_scenes),
            Arc::new(sweep),
        ],
    )
}

pub struct OutlineFeature;

impl Feature for OutlineFeature {
    const NAME: &'static str = "outline";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced { events, outbox } =
            EventSourced::in_memory(outline_messaging::message_for, context);

        Ports {
            events,
            catalog: Arc::new(InMemoryOutlineCatalog::new()),
            registry: Arc::new(InMemoryRegistry::new()),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_postgres(
            &pool,
            outline_store::codec(),
            outline_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            catalog: Arc::new(outline_catalog::PostgresOutlineCatalog::new(pool.clone())),
            registry: Arc::new(registry::PostgresRegistry::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::postgres::migrations(),
            outbox::postgres::migrations(),
            outline_catalog::postgres::migrations(),
            registry::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_sqlite(
            &pool,
            outline_store::codec(),
            outline_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            catalog: Arc::new(outline_catalog::SqliteOutlineCatalog::new(pool.clone())),
            registry: Arc::new(registry::SqliteRegistry::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::sqlite::migrations(),
            outbox::sqlite::migrations(),
            outline_catalog::sqlite::migrations(),
            registry::sqlite::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
