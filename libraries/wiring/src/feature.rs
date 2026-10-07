use std::sync::Arc;

use axum::Router;
use eventsourcing::Outbox;
use messaging::Listener;

use crate::{Context, Unprepared, Wired};

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

    fn outbox(ports: &Self::Ports) -> Option<Arc<dyn Outbox>>;

    fn wire(ports: &Self::Ports, context: &Context) -> Wired;
}

pub enum Storage {
    InMemory,
    #[cfg(feature = "postgres")]
    Postgres(Arc<dyn crate::Databases>),
}

pub struct Assembled {
    pub outbox: Option<Arc<dyn Outbox>>,
    pub routes: Router,
    pub listeners: Vec<Arc<dyn Listener>>,
}

pub async fn assemble<F: Feature>(
    storage: &Storage,
    context: &Context,
) -> Result<Assembled, Unprepared> {
    let ports = match storage {
        Storage::InMemory => F::in_memory(context),
        #[cfg(feature = "postgres")]
        Storage::Postgres(databases) => {
            let pool = databases.ready(F::NAME).await?;
            for schema in F::postgres_schema() {
                crate::database::lay_out(F::NAME, &pool, schema).await?;
            }

            F::on_postgres(pool, context)?
        }
    };
    let wired = F::wire(&ports, context);

    Ok(Assembled {
        outbox: F::outbox(&ports),
        routes: wired.routes,
        listeners: wired.listeners,
    })
}
