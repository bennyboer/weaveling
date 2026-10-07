use std::sync::Arc;

use appearances_catalog::InMemoryAppearanceCatalog;
use appearances_core::AppearanceCatalog;
use appearances_messaging::{
    ForgetDiscardedIdea, OutlineAppearancesProjector, PassageAppearancesProjector,
};
use outbox::Outbox;
use wiring::{Context, Feature, Wired};

pub struct Ports {
    pub catalog: Arc<dyn AppearanceCatalog>,
}

impl Ports {
    pub fn in_memory() -> Self {
        Self {
            catalog: Arc::new(InMemoryAppearanceCatalog::new()),
        }
    }

    #[cfg(feature = "postgres")]
    pub fn postgres(pool: sqlx::PgPool) -> Self {
        Self {
            catalog: Arc::new(appearances_catalog::PostgresAppearanceCatalog::new(pool)),
        }
    }
}

pub fn wire(ports: &Ports) -> Wired {
    Wired::serving(appearances_rest::router(ports.catalog.clone())).listening(vec![
        Arc::new(OutlineAppearancesProjector::new(ports.catalog.clone())),
        Arc::new(PassageAppearancesProjector::new(ports.catalog.clone())),
        Arc::new(ForgetDiscardedIdea::new(ports.catalog.clone())),
    ])
}

pub struct AppearanceFeature;

impl Feature for AppearanceFeature {
    const NAME: &'static str = "appearances";

    type Ports = Ports;

    fn in_memory(_context: &Context) -> Ports {
        Ports::in_memory()
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, _context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports::postgres(pool))
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![appearances_catalog::migrations()]
    }

    fn outbox(_ports: &Ports) -> Option<Arc<dyn Outbox>> {
        None
    }

    fn wire(ports: &Ports, _context: &Context) -> Wired {
        wire(ports)
    }
}
