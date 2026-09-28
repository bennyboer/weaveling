use std::sync::Arc;

use eventsourcing::{EventStore, InMemoryEventStore, PublishingEventStore};
use projects_catalog::InMemoryProjectCatalog;
use projects_core::{ProjectCatalog, ProjectEvent, ProjectService};
use projects_messaging::{ProjectCatalogProjector, ProjectEventPublisher};
use wiring::{Context, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<ProjectEvent>>,
    pub catalog: Arc<dyn ProjectCatalog>,
}

impl Ports {
    pub fn in_memory(publisher: Arc<dyn messaging::Publisher>) -> Self {
        Self {
            events: PublishingEventStore::wrapping(
                Arc::new(InMemoryEventStore::new()),
                Arc::new(ProjectEventPublisher::new(publisher)),
            ),
            catalog: Arc::new(InMemoryProjectCatalog::new()),
        }
    }

    #[cfg(feature = "postgres")]
    pub fn postgres(pool: sqlx::PgPool) -> Self {
        use eventsourcing::PostgresEventStore;

        Self {
            events: Arc::new(PostgresEventStore::new(
                pool.clone(),
                projects_store::codec(),
                projects_messaging::message_for,
            )),
            catalog: Arc::new(projects_catalog::PostgresProjectCatalog::new(pool)),
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
