use std::sync::Arc;

use eventsourcing::{EventStore, InMemoryEventStore};
use outbox::{InMemoryOutbox, Outbox};
use projects_catalog::InMemoryProjectCatalog;
use projects_core::{ProjectCatalog, ProjectEvent, ProjectService};
use projects_messaging::ProjectCatalogProjector;
use wiring::{Context, Feature, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<ProjectEvent>>,
    pub catalog: Arc<dyn ProjectCatalog>,
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
                projects_messaging::message_for,
            )),
            catalog: Arc::new(InMemoryProjectCatalog::new()),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    pub fn postgres(
        pool: sqlx::PgPool,
        publisher: Arc<dyn messaging::Publisher>,
        clock: Arc<dyn clock::Clock>,
    ) -> Self {
        use eventsourcing::PostgresEventStore;
        use outbox::PostgresOutbox;

        Self {
            events: Arc::new(PostgresEventStore::new(
                pool.clone(),
                projects_store::codec(),
                projects_messaging::message_for,
            )),
            catalog: Arc::new(projects_catalog::PostgresProjectCatalog::new(pool.clone())),
            outbox: Arc::new(PostgresOutbox::new(pool, publisher, clock)),
        }
    }
}

pub fn service(ports: &Ports, context: &Context) -> ProjectService {
    ProjectService::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let projector = ProjectCatalogProjector::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    );

    Wired::serving(projects_rest::router(service(ports, context)))
        .listening(vec![Arc::new(projector)])
}

pub struct ProjectFeature;

impl Feature for ProjectFeature {
    const NAME: &'static str = "projects";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        Ports::in_memory(context.publisher.clone(), context.clock.clone())
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports::postgres(
            pool,
            context.publisher.clone(),
            context.clock.clone(),
        ))
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::migrations(),
            outbox::postgres::migrations(),
            projects_catalog::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
