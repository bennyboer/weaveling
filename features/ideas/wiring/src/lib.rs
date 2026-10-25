use std::sync::Arc;

use eventsourcing::EventStore;
use ideas_catalog::InMemoryIdeaCatalog;
use ideas_core::{IdeaCatalog, IdeaEvent, IdeasCore};
use ideas_messaging::{
    BroadcastIdeaChangesListener, CatalogueIdeasListener, DiscardOnProjectDeletedListener,
};
use live::Broadcaster;
use outbox::Outbox;
use wiring::{Context, EventSourced, Feature, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<IdeaEvent>>,
    pub catalog: Arc<dyn IdeaCatalog>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn core(ports: &Ports, context: &Context) -> IdeasCore {
    IdeasCore::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.authorizer.clone(),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let ideas = core(ports, context);
    let projector = CatalogueIdeasListener::new(ideas.clone());
    let sweep = DiscardOnProjectDeletedListener::new(
        ideas.clone(),
        context.publisher.clone(),
        context.clock.clone(),
    );

    let broadcaster = Broadcaster::new();
    let broadcast = BroadcastIdeaChangesListener::new(ideas.clone(), broadcaster.clone());

    Wired::serving(ideas_rest::router(ideas.clone()).merge(ideas_rest::live(ideas, broadcaster)))
        .listening(vec![
            Arc::new(broadcast),
            Arc::new(projector),
            Arc::new(sweep),
        ])
}

pub struct IdeaFeature;

impl Feature for IdeaFeature {
    const NAME: &'static str = "ideas";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced { events, outbox } =
            EventSourced::in_memory(ideas_messaging::message_for, context);

        Ports {
            events,
            catalog: Arc::new(InMemoryIdeaCatalog::new()),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_postgres(
            &pool,
            ideas_store::codec(),
            ideas_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            catalog: Arc::new(ideas_catalog::PostgresIdeaCatalog::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::postgres::migrations(),
            outbox::postgres::migrations(),
            ideas_catalog::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_sqlite(
            &pool,
            ideas_store::codec(),
            ideas_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            catalog: Arc::new(ideas_catalog::SqliteIdeaCatalog::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::sqlite::migrations(),
            outbox::sqlite::migrations(),
            ideas_catalog::sqlite::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
