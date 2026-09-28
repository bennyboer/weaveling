use std::sync::Arc;

use eventsourcing::{EventStore, InMemoryEventStore, InMemoryOutbox, Outbox};
use projects_catalog::InMemoryProjectCatalog;
use projects_core::{ProjectCatalog, ProjectEvent, ProjectService};
use projects_messaging::ProjectCatalogProjector;
use wiring::{Context, Wired};

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
        use eventsourcing::{PostgresEventStore, PostgresOutbox};

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

pub const NAME: &str = "projects";

#[cfg(feature = "postgres")]
pub async fn lay_out(pool: &sqlx::PgPool) -> Result<(), wiring::Unprepared> {
    wiring::database::lay_out(NAME, pool, eventsourcing::migrations()).await?;
    wiring::database::lay_out(NAME, pool, projects_catalog::migrations()).await
}
