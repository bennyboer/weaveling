use std::sync::Arc;

use async_trait::async_trait;

use outbox::{InMemoryOutbox, Outbox};
use scenes_catalog::InMemorySceneCatalog;
use scenes_core::{SceneCatalog, SceneService, SceneStore};
use scenes_messaging::{CatalogueScenes, DeleteOnProjectDeleted, UnlinkOnDiscard};
use scenes_store::InMemorySceneStore;
use scenes_sync::LiveScenes;
use wiring::{Context, Feature, Settling, Wired};

pub struct Ports {
    pub store: Arc<dyn SceneStore>,
    pub catalog: Arc<dyn SceneCatalog>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let scenes = SceneService::new(ports.store.clone(), context.clock.clone());
    let unlink = UnlinkOnDiscard::new(scenes.clone());
    let sweep = DeleteOnProjectDeleted::new(
        scenes.clone(),
        context.publisher.clone(),
        context.clock.clone(),
    );
    let catalogue = CatalogueScenes::new(ports.catalog.clone());

    let live = LiveScenes::new(scenes.clone());

    Wired::serving(
        scenes_rest::router(scenes, ports.catalog.clone()).merge(scenes_sync::router(live.clone())),
    )
    .listening(vec![Arc::new(sweep), Arc::new(unlink), Arc::new(catalogue)])
    .settling_before_stopping(vec![Arc::new(SettlingLiveScenes(live))])
}

struct SettlingLiveScenes(LiveScenes);

#[async_trait]
impl Settling for SettlingLiveScenes {
    async fn settle(&self) {
        self.0.settle().await;
    }
}

pub struct SceneFeature;

impl Feature for SceneFeature {
    const NAME: &'static str = "scenes";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let outbox = Arc::new(InMemoryOutbox::new(
            context.publisher.clone(),
            context.clock.clone(),
        ));

        Ports {
            store: Arc::new(InMemorySceneStore::new().enqueuing_to(
                outbox.clone(),
                context.clock.clone(),
                scenes_messaging::message_for,
            )),
            catalog: Arc::new(InMemorySceneCatalog::new()),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            store: Arc::new(
                scenes_store::PostgresSceneStore::new(pool.clone())
                    .enqueuing(context.clock.clone(), scenes_messaging::message_for),
            ),
            catalog: Arc::new(scenes_catalog::PostgresSceneCatalog::new(pool.clone())),
            outbox: Arc::new(outbox::PostgresOutbox::new(
                pool,
                context.publisher.clone(),
                context.clock.clone(),
            )),
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            outbox::postgres::migrations(),
            scenes_store::postgres::migrations(),
            scenes_catalog::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            store: Arc::new(
                scenes_store::SqliteSceneStore::new(pool.clone())
                    .enqueuing(context.clock.clone(), scenes_messaging::message_for),
            ),
            catalog: Arc::new(scenes_catalog::SqliteSceneCatalog::new(pool.clone())),
            outbox: Arc::new(outbox::SqliteOutbox::new(
                pool,
                context.publisher.clone(),
                context.clock.clone(),
            )),
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            outbox::sqlite::migrations(),
            scenes_store::sqlite::migrations(),
            scenes_catalog::sqlite::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
