use std::sync::Arc;

use outbox::Outbox;
use readings_core::ReadingsCore;
use wiring::{Context, Feature, Wired};

pub struct Ports;

pub fn wire(_ports: &Ports, _context: &Context) -> Wired {
    Wired::serving(readings_rest::router(ReadingsCore::new()))
}

pub struct ReadingsFeature;

impl Feature for ReadingsFeature {
    const NAME: &'static str = "readings";

    type Ports = Ports;

    fn in_memory(_context: &Context) -> Ports {
        Ports
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(_pool: sqlx::PgPool, _context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports)
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(_pool: sqlx::SqlitePool, _context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports)
    }

    fn outbox(_ports: &Ports) -> Option<Arc<dyn Outbox>> {
        None
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
