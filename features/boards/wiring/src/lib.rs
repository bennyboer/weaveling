use std::sync::Arc;

use boards_catalog::InMemoryBoardCatalog;
use boards_core::{BoardCatalog, BoardEvent, BoardService};
use boards_messaging::{
    BoardCatalogProjector, DiscardBoardsOnProjectDeleted, PinnedIdeasProjector, UnpinOnDiscard,
};
use eventsourcing::EventStore;
use outbox::Outbox;
use registry::{InMemoryRegistry, Registry};
use wiring::{Context, EventSourced, Feature, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<BoardEvent>>,
    pub catalog: Arc<dyn BoardCatalog>,
    pub registry: Arc<dyn Registry>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn service(ports: &Ports, context: &Context) -> BoardService {
    BoardService::new(
        ports.events.clone(),
        ports.registry.clone(),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let boards = service(ports, context);
    let catalogue = BoardCatalogProjector::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    );
    let index = PinnedIdeasProjector::new(
        ports.events.clone(),
        ports.catalog.clone(),
        boards.clone(),
        context.clock.clone(),
    );
    let unpin_discarded_ideas = UnpinOnDiscard::new(boards.clone(), ports.catalog.clone());
    let sweep = DiscardBoardsOnProjectDeleted::new(boards.clone());

    Wired::serving(boards_rest::router(boards)).listening(vec![
        Arc::new(catalogue),
        Arc::new(index),
        Arc::new(unpin_discarded_ideas),
        Arc::new(sweep),
    ])
}

pub struct BoardFeature;

impl Feature for BoardFeature {
    const NAME: &'static str = "boards";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced { events, outbox } =
            EventSourced::in_memory(boards_messaging::message_for, context);

        Ports {
            events,
            catalog: Arc::new(InMemoryBoardCatalog::new()),
            registry: Arc::new(InMemoryRegistry::new()),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_postgres(
            &pool,
            boards_store::codec(),
            boards_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            catalog: Arc::new(boards_catalog::PostgresBoardCatalog::new(pool.clone())),
            registry: Arc::new(registry::PostgresRegistry::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::postgres::migrations(),
            outbox::postgres::migrations(),
            boards_catalog::postgres::migrations(),
            registry::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_sqlite(
            &pool,
            boards_store::codec(),
            boards_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            catalog: Arc::new(boards_catalog::SqliteBoardCatalog::new(pool.clone())),
            registry: Arc::new(registry::SqliteRegistry::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::sqlite::migrations(),
            outbox::sqlite::migrations(),
            boards_catalog::sqlite::migrations(),
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
