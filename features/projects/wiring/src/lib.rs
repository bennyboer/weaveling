use std::sync::Arc;

use eventsourcing::EventStore;
use live::{BroadcastChangesListener, Broadcaster};
use outbox::Outbox;
use projects_catalog::InMemoryProjectCatalog;
use projects_contract::EVERY_PROJECT;
use projects_core::{ProjectCatalog, ProjectEvent, ProjectsCore};
use projects_messaging::CatalogueProjectsListener;
use wiring::{Context, EventSourced, Feature, Wired};

pub const LIVE: &str = "broadcast-project-changes";

pub struct Ports {
    pub events: Arc<dyn EventStore<ProjectEvent>>,
    pub catalog: Arc<dyn ProjectCatalog>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn core(ports: &Ports, context: &Context) -> ProjectsCore {
    ProjectsCore::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.authorizer.clone(),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let projects = core(ports, context);
    let projector = CatalogueProjectsListener::new(projects.clone());

    let broadcaster = Broadcaster::new();
    let broadcast = BroadcastChangesListener::under(
        LIVE,
        EVERY_PROJECT,
        projects_rest::FOLLOWED,
        broadcaster.clone(),
    );

    Wired::serving(
        projects_rest::router(projects.clone()).merge(projects_rest::live(projects, broadcaster)),
    )
    .listening(vec![Arc::new(broadcast), Arc::new(projector)])
}

pub struct ProjectFeature;

impl Feature for ProjectFeature {
    const NAME: &'static str = "projects";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced { events, outbox } =
            EventSourced::in_memory(projects_messaging::message_for, context);

        Ports {
            events,
            catalog: Arc::new(InMemoryProjectCatalog::new()),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_postgres(
            &pool,
            projects_store::codec(),
            projects_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            catalog: Arc::new(projects_catalog::PostgresProjectCatalog::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::postgres::migrations(),
            outbox::postgres::migrations(),
            projects_catalog::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_sqlite(
            &pool,
            projects_store::codec(),
            projects_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            catalog: Arc::new(projects_catalog::SqliteProjectCatalog::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::sqlite::migrations(),
            outbox::sqlite::migrations(),
            projects_catalog::sqlite::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
