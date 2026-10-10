use std::sync::Arc;

use outbox::{InMemoryOutbox, Outbox};
use scenes_core::{SceneService, SceneStore};
use scenes_messaging::{DeleteOnProjectDeleted, UnlinkOnDiscard};
use scenes_store::InMemorySceneStore;
use scenes_sync::LiveScenes;
use wiring::{Context, Feature, Wired};

pub struct Ports {
    pub store: Arc<dyn SceneStore>,
    pub outbox: Arc<dyn Outbox>,
}

impl Ports {
    pub fn in_memory(
        publisher: Arc<dyn messaging::Publisher>,
        clock: Arc<dyn clock::Clock>,
    ) -> Self {
        let outbox = Arc::new(InMemoryOutbox::new(publisher, clock.clone()));

        Self {
            store: Arc::new(InMemorySceneStore::new().enqueuing_to(
                outbox.clone(),
                clock,
                scenes_messaging::message_for,
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
        use scenes_store::PostgresSceneStore;

        Self {
            store: Arc::new(
                PostgresSceneStore::new(pool.clone())
                    .enqueuing(clock.clone(), scenes_messaging::message_for),
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
        use scenes_store::SqliteSceneStore;

        Self {
            store: Arc::new(
                SqliteSceneStore::new(pool.clone())
                    .enqueuing(clock.clone(), scenes_messaging::message_for),
            ),
            outbox: Arc::new(SqliteOutbox::new(pool, publisher, clock)),
        }
    }
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let scenes = SceneService::new(ports.store.clone(), context.clock.clone());
    let unlink = UnlinkOnDiscard::new(scenes.clone());
    let sweep = DeleteOnProjectDeleted::new(
        scenes.clone(),
        context.publisher.clone(),
        context.clock.clone(),
    );

    Wired::serving(
        scenes_rest::router(scenes.clone()).merge(scenes_sync::router(LiveScenes::new(scenes))),
    )
    .listening(vec![Arc::new(sweep), Arc::new(unlink)])
}

pub struct SceneFeature;

impl Feature for SceneFeature {
    const NAME: &'static str = "scenes";

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
            scenes_store::postgres::migrations(),
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
            scenes_store::sqlite::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
