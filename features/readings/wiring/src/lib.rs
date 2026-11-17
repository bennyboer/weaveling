use std::sync::Arc;

use access::{InMemoryAccess, KeptAccess};
use authorizing::{AllowingEverything, Authorizer};
use outbox::Outbox;
use readings_catalog::SampleBook;
use readings_core::{OutlineCatalog, ReadingsCore, SceneTextCatalog};
use transacting::{InMemoryTransactions, Transactions};
use wiring::{Context, Feature, Wired};

pub struct Ports {
    pub outlines: Arc<dyn OutlineCatalog>,
    pub texts: Arc<dyn SceneTextCatalog>,
    pub transactions: Arc<dyn Transactions>,
    pub access: KeptAccess,
}

impl Ports {
    fn kept_in(transactions: Arc<dyn Transactions>, access: KeptAccess) -> Self {
        // TODO: catalogs kept from the outline's and the scenes' messages, in memory and in the databases
        Self {
            outlines: Arc::new(SampleBook),
            texts: Arc::new(SampleBook),
            transactions,
            access,
        }
    }
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    // TODO: listeners that remember the members' roles and the trash, and the chores that forget old records
    let authorizer: Arc<dyn Authorizer> = if context.with_accounts {
        Arc::new(ports.access.clone())
    } else {
        Arc::new(AllowingEverything)
    };

    let readings = ReadingsCore::new(
        ports.outlines.clone(),
        ports.texts.clone(),
        ports.transactions.clone(),
        ports.access.clone(),
        authorizer,
        context.clock.clone(),
    );

    Wired::serving(readings_rest::router(readings))
}

pub struct ReadingsFeature;

impl Feature for ReadingsFeature {
    const NAME: &'static str = "readings";

    type Ports = Ports;

    fn in_memory(_context: &Context) -> Ports {
        Ports::kept_in(
            Arc::new(InMemoryTransactions::new()),
            KeptAccess::new(Arc::new(InMemoryAccess::new())),
        )
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, _context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports::kept_in(
            Arc::new(transacting::PostgresTransactions::new(pool.clone())),
            KeptAccess::new(Arc::new(access::PostgresAccess::new(pool))),
        ))
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![access::postgres::migrations()]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, _context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports::kept_in(
            Arc::new(transacting::SqliteTransactions::new(pool.clone())),
            KeptAccess::new(Arc::new(access::SqliteAccess::new(pool))),
        ))
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![access::sqlite::migrations()]
    }

    fn outbox(_ports: &Ports) -> Option<Arc<dyn Outbox>> {
        None
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
