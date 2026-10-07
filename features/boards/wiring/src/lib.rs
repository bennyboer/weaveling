use std::sync::Arc;

use boards_catalog::InMemoryBoardCatalog;
use boards_core::{BoardCatalog, BoardEvent, BoardService};
use boards_messaging::{
    BoardCatalogProjector, DiscardBoardsOnProjectDeleted, PinnedIdeasProjector, UnpinOnDiscard,
};
use eventsourcing::{EventStore, InMemoryEventStore};
use outbox::{InMemoryOutbox, Outbox};
use registry::{InMemoryRegistry, Registry};
use wiring::{Context, Feature, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<BoardEvent>>,
    pub catalog: Arc<dyn BoardCatalog>,
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
                boards_messaging::message_for,
            )),
            catalog: Arc::new(InMemoryBoardCatalog::new()),
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
        use eventsourcing::PostgresEventStore;
        use outbox::PostgresOutbox;

        Self {
            events: Arc::new(PostgresEventStore::new(
                pool.clone(),
                boards_store::codec(),
                boards_messaging::message_for,
            )),
            catalog: Arc::new(boards_catalog::PostgresBoardCatalog::new(pool.clone())),
            registry: Arc::new(registry::PostgresRegistry::new(pool.clone())),
            outbox: Arc::new(PostgresOutbox::new(pool, publisher, clock)),
        }
    }

    #[cfg(feature = "sqlite")]
    pub fn sqlite(
        pool: sqlx::SqlitePool,
        publisher: Arc<dyn messaging::Publisher>,
        clock: Arc<dyn clock::Clock>,
    ) -> Self {
        use eventsourcing::SqliteEventStore;
        use outbox::SqliteOutbox;

        Self {
            events: Arc::new(SqliteEventStore::new(
                pool.clone(),
                boards_store::codec(),
                boards_messaging::message_for,
            )),
            catalog: Arc::new(boards_catalog::SqliteBoardCatalog::new(pool.clone())),
            registry: Arc::new(registry::SqliteRegistry::new(pool.clone())),
            outbox: Arc::new(SqliteOutbox::new(pool, publisher, clock)),
        }
    }
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
        context.clock.clone(),
    );
    let tidy = UnpinOnDiscard::new(boards.clone(), ports.catalog.clone());
    let sweep = DiscardBoardsOnProjectDeleted::new(boards.clone());

    Wired::serving(boards_rest::router(boards)).listening(vec![
        Arc::new(catalogue),
        Arc::new(index),
        Arc::new(tidy),
        Arc::new(sweep),
    ])
}

pub struct BoardFeature;

impl Feature for BoardFeature {
    const NAME: &'static str = "boards";

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
            eventsourcing::postgres::migrations(),
            outbox::postgres::migrations(),
            boards_catalog::postgres::migrations(),
            registry::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports::sqlite(
            pool,
            context.publisher.clone(),
            context.clock.clone(),
        ))
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
