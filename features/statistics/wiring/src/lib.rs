use std::sync::Arc;

use live::Broadcaster;
use outbox::Outbox;
use statistics_catalog::InMemoryWordCountCatalog;
use statistics_core::WordCountCatalog;
use statistics_messaging::ProjectWordCountsListener;
use wiring::{Context, Feature, Wired};

pub struct Ports {
    pub catalog: Arc<dyn WordCountCatalog>,
}

pub fn wire(ports: &Ports) -> Wired {
    let broadcaster = Broadcaster::new();
    let telling: Arc<dyn WordCountCatalog> = Arc::new(statistics_rest::TellingWordCounts::around(
        ports.catalog.clone(),
        broadcaster.clone(),
    ));

    Wired::serving(
        statistics_rest::router(ports.catalog.clone()).merge(statistics_rest::live(broadcaster)),
    )
    .listening(vec![Arc::new(ProjectWordCountsListener::new(telling))])
}

pub struct StatisticsFeature;

impl Feature for StatisticsFeature {
    const NAME: &'static str = "statistics";

    type Ports = Ports;

    fn in_memory(_context: &Context) -> Ports {
        Ports {
            catalog: Arc::new(InMemoryWordCountCatalog::new()),
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, _context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            catalog: Arc::new(statistics_catalog::PostgresWordCountCatalog::new(pool)),
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![statistics_catalog::postgres::migrations()]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, _context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            catalog: Arc::new(statistics_catalog::SqliteWordCountCatalog::new(pool)),
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![statistics_catalog::sqlite::migrations()]
    }

    fn outbox(_ports: &Ports) -> Option<Arc<dyn Outbox>> {
        None
    }

    fn wire(ports: &Ports, _context: &Context) -> Wired {
        wire(ports)
    }
}
