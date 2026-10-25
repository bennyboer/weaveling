use std::sync::Arc;

use appearances_catalog::InMemoryAppearanceCatalog;
use appearances_core::AppearanceCatalog;
use appearances_messaging::{
    ForgetDiscardedIdeaListener, ProjectOutlineAppearancesListener, ProjectSceneAppearancesListener,
};
use appearances_rest::Readers;
use live::Broadcaster;
use outbox::Outbox;
use wiring::{Context, Feature, Wired};

pub struct Ports {
    pub catalog: Arc<dyn AppearanceCatalog>,
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let broadcaster = Broadcaster::new();
    let telling: Arc<dyn AppearanceCatalog> = Arc::new(
        appearances_rest::TellingAppearances::around(ports.catalog.clone(), broadcaster.clone()),
    );

    Wired::serving(
        appearances_rest::router(readers(ports, context))
            .merge(appearances_rest::live(broadcaster, readers(ports, context))),
    )
    .listening(vec![
        Arc::new(ProjectOutlineAppearancesListener::new(telling.clone())),
        Arc::new(ProjectSceneAppearancesListener::new(telling.clone())),
        Arc::new(ForgetDiscardedIdeaListener::new(telling)),
    ])
}

fn readers(ports: &Ports, context: &Context) -> Readers {
    Readers {
        catalog: ports.catalog.clone(),
        authorizer: context.authorizer.clone(),
    }
}

pub struct AppearanceFeature;

impl Feature for AppearanceFeature {
    const NAME: &'static str = "appearances";

    type Ports = Ports;

    fn in_memory(_context: &Context) -> Ports {
        Ports {
            catalog: Arc::new(InMemoryAppearanceCatalog::new()),
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, _context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            catalog: Arc::new(appearances_catalog::PostgresAppearanceCatalog::new(pool)),
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![appearances_catalog::postgres::migrations()]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, _context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            catalog: Arc::new(appearances_catalog::SqliteAppearanceCatalog::new(pool)),
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![appearances_catalog::sqlite::migrations()]
    }

    fn outbox(_ports: &Ports) -> Option<Arc<dyn Outbox>> {
        None
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
