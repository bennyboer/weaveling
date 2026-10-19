use std::sync::Arc;

use appearances_catalog::InMemoryAppearanceCatalog;
use appearances_core::AppearanceCatalog;
use appearances_messaging::{
    ForgetDiscardedIdeaListener, ProjectOutlineAppearancesListener, ProjectSceneAppearancesListener,
};
use outbox::Outbox;
use wiring::{Context, Feature, Wired};

pub struct Ports {
    pub catalog: Arc<dyn AppearanceCatalog>,
}

pub fn wire(ports: &Ports) -> Wired {
    Wired::serving(appearances_rest::router(ports.catalog.clone())).listening(vec![
        Arc::new(ProjectOutlineAppearancesListener::new(
            ports.catalog.clone(),
        )),
        Arc::new(ProjectSceneAppearancesListener::new(ports.catalog.clone())),
        Arc::new(ForgetDiscardedIdeaListener::new(ports.catalog.clone())),
    ])
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

    fn wire(ports: &Ports, _context: &Context) -> Wired {
        wire(ports)
    }
}
