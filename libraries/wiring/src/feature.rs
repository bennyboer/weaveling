use std::sync::Arc;

use axum::Router;
use live::Topic;
use messaging::Listener;
use outbox::Outbox;

use chores::Chore;

use crate::{Context, Settling, Unprepared, Wired};

pub trait Feature {
    const NAME: &'static str;

    type Ports;

    fn in_memory(context: &Context) -> Self::Ports;

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Self::Ports, Unprepared> {
        let _ = (pool, context);

        Err(Unprepared::NotBuiltIn {
            feature: Self::NAME,
            backend: "PostgreSQL",
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        Vec::new()
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Self::Ports, Unprepared> {
        let _ = (pool, context);

        Err(Unprepared::NotBuiltIn {
            feature: Self::NAME,
            backend: "SQLite",
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        Vec::new()
    }

    fn outbox(ports: &Self::Ports) -> Option<Arc<dyn Outbox>>;

    fn wire(ports: &Self::Ports, context: &Context) -> Wired;
}

pub enum Storage {
    InMemory,
    #[cfg(feature = "postgres")]
    Postgres(Arc<dyn crate::Databases>),
    #[cfg(feature = "sqlite")]
    Sqlite(Arc<dyn crate::sqlite::Files>),
}

pub struct Assembled {
    pub outbox: Option<Arc<dyn Outbox>>,
    pub routes: Router,
    pub open_routes: Router,
    pub topics: Vec<Arc<dyn Topic>>,
    pub listeners: Vec<Arc<dyn Listener>>,
    pub settling: Vec<Arc<dyn Settling>>,
    pub chores: Vec<Arc<dyn Chore>>,
}

pub async fn assemble<F: Feature>(
    storage: &Storage,
    context: &Context,
) -> Result<Assembled, Unprepared> {
    let ports = ports::<F>(storage, context).await?;
    let wired = F::wire(&ports, context);

    Ok(Assembled {
        outbox: F::outbox(&ports),
        routes: wired.routes,
        open_routes: wired.open_routes,
        topics: wired.topics,
        listeners: wired.listeners,
        settling: wired.settling,
        chores: wired.chores,
    })
}

pub async fn ports<F: Feature>(
    storage: &Storage,
    context: &Context,
) -> Result<F::Ports, Unprepared> {
    Ok(match storage {
        Storage::InMemory => F::in_memory(context),
        #[cfg(feature = "postgres")]
        Storage::Postgres(databases) => {
            let pool = databases.ready(F::NAME).await?;
            for schema in F::postgres_schema() {
                crate::postgres::lay_out(F::NAME, &pool, schema).await?;
            }

            F::on_postgres(pool, context)?
        }
        #[cfg(feature = "sqlite")]
        Storage::Sqlite(files) => {
            let pool = files.ready(F::NAME).await?;
            for schema in F::sqlite_schema() {
                crate::sqlite::lay_out(F::NAME, &pool, schema).await?;
            }

            F::on_sqlite(pool, context)?
        }
    })
}
