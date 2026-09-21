use std::sync::Arc;

use passages_core::{PassageService, PassageStore};
use passages_store::InMemoryPassageStore;
use passages_sync::LivePassages;
use wiring::{Context, Wired};

pub struct Ports {
    pub store: Arc<dyn PassageStore>,
}

impl Ports {
    pub fn in_memory() -> Self {
        Self {
            store: Arc::new(InMemoryPassageStore::new()),
        }
    }

    #[cfg(feature = "postgres")]
    pub fn postgres(pool: sqlx::PgPool) -> Self {
        Self {
            store: Arc::new(passages_store::PostgresPassageStore::new(pool)),
        }
    }
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let passages = PassageService::new(ports.store.clone(), context.clock.clone());

    Wired::serving(
        passages_rest::router(passages.clone())
            .merge(passages_sync::router(LivePassages::new(passages))),
    )
}

pub const NAME: &str = "passages";

#[cfg(feature = "postgres")]
pub async fn lay_out(pool: &sqlx::PgPool) -> Result<(), wiring::Unprepared> {
    wiring::database::lay_out(NAME, pool, passages_store::migrations()).await
}

#[cfg(feature = "postgres")]
pub fn outbox(
    _pool: &sqlx::PgPool,
    _publisher: Arc<dyn messaging::Publisher>,
    _clock: Arc<dyn clock::Clock>,
) -> Option<eventsourcing::PostgresOutbox> {
    None
}
