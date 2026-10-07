use std::sync::Arc;

use outbox::{InMemoryOutbox, Outbox};
use passages_core::{PassageService, PassageStore};
use passages_messaging::{DeleteOnProjectDeleted, UnlinkOnDiscard};
use passages_store::InMemoryPassageStore;
use passages_sync::LivePassages;
use wiring::{Context, Feature, Wired};

pub struct Ports {
    pub store: Arc<dyn PassageStore>,
    pub outbox: Arc<dyn Outbox>,
}

impl Ports {
    pub fn in_memory(
        publisher: Arc<dyn messaging::Publisher>,
        clock: Arc<dyn clock::Clock>,
    ) -> Self {
        let outbox = Arc::new(InMemoryOutbox::new(publisher, clock.clone()));

        Self {
            store: Arc::new(InMemoryPassageStore::new().enqueuing_to(
                outbox.clone(),
                clock,
                passages_messaging::message_for,
            )),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    pub fn postgres(
        pool: sqlx::PgPool,
        publisher: Arc<dyn messaging::Publisher>,
        clock: Arc<dyn clock::Clock>,
    ) -> Self {
        use outbox::PostgresOutbox;
        use passages_store::PostgresPassageStore;

        Self {
            store: Arc::new(
                PostgresPassageStore::new(pool.clone())
                    .enqueuing(clock.clone(), passages_messaging::message_for),
            ),
            outbox: Arc::new(PostgresOutbox::new(pool, publisher, clock)),
        }
    }

    #[cfg(feature = "sqlite")]
    pub fn sqlite(
        pool: sqlx::SqlitePool,
        publisher: Arc<dyn messaging::Publisher>,
        clock: Arc<dyn clock::Clock>,
    ) -> Self {
        use outbox::SqliteOutbox;
        use passages_store::SqlitePassageStore;

        Self {
            store: Arc::new(
                SqlitePassageStore::new(pool.clone())
                    .enqueuing(clock.clone(), passages_messaging::message_for),
            ),
            outbox: Arc::new(SqliteOutbox::new(pool, publisher, clock)),
        }
    }
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let passages = PassageService::new(ports.store.clone(), context.clock.clone());
    let unlink = UnlinkOnDiscard::new(passages.clone());
    let sweep = DeleteOnProjectDeleted::new(
        passages.clone(),
        context.publisher.clone(),
        context.clock.clone(),
    );

    Wired::serving(
        passages_rest::router(passages.clone())
            .merge(passages_sync::router(LivePassages::new(passages))),
    )
    .listening(vec![Arc::new(sweep), Arc::new(unlink)])
}

pub struct PassageFeature;

impl Feature for PassageFeature {
    const NAME: &'static str = "passages";

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
            outbox::postgres::migrations(),
            passages_store::postgres::migrations(),
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
            outbox::sqlite::migrations(),
            passages_store::sqlite::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
