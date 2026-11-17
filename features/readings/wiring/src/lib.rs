use std::sync::Arc;

use outbox::Outbox;
use readings_catalog::SampleBook;
use readings_core::{OutlineCatalog, ReadingsCore, SceneTextCatalog};
use wiring::{Context, Feature, Wired};

pub struct Ports {
    pub outlines: Arc<dyn OutlineCatalog>,
    pub texts: Arc<dyn SceneTextCatalog>,
}

impl Ports {
    fn sample() -> Self {
        // TODO: catalogs kept from the outline's and the scenes' messages, in memory and in the databases
        Self {
            outlines: Arc::new(SampleBook),
            texts: Arc::new(SampleBook),
        }
    }
}

pub fn wire(ports: &Ports, _context: &Context) -> Wired {
    Wired::serving(readings_rest::router(ReadingsCore::new(
        ports.outlines.clone(),
        ports.texts.clone(),
    )))
}

pub struct ReadingsFeature;

impl Feature for ReadingsFeature {
    const NAME: &'static str = "readings";

    type Ports = Ports;

    fn in_memory(_context: &Context) -> Ports {
        Ports::sample()
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(_pool: sqlx::PgPool, _context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports::sample())
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(_pool: sqlx::SqlitePool, _context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports::sample())
    }

    fn outbox(_ports: &Ports) -> Option<Arc<dyn Outbox>> {
        None
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
