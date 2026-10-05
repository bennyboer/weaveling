use std::sync::Arc;

use eventsourcing::{InMemoryOutbox, Outbox};
use passages_core::{PassageService, PassageStore};
use passages_messaging::{DeleteOnProjectDeleted, UnlinkOnDiscard};
use passages_store::InMemoryPassageStore;
use passages_sync::LivePassages;
use wiring::{Context, Wired};

pub struct Ports {
    pub store: Arc<dyn PassageStore>,
    pub outbox: Arc<dyn Outbox>,
}

impl Ports {
    pub fn in_memory(
        publisher: Arc<dyn messaging::Publisher>,
        clock: Arc<dyn clock::Clock>,
    ) -> Self {
        let outbox = Arc::new(InMemoryOutbox::new(publisher, clock.clone()));

        Self {
            store: Arc::new(InMemoryPassageStore::new().enqueuing_to(
                outbox.clone(),
                clock,
                passages_messaging::message_for,
            )),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    pub fn postgres(
        pool: sqlx::PgPool,
        publisher: Arc<dyn messaging::Publisher>,
        clock: Arc<dyn clock::Clock>,
    ) -> Self {
        use eventsourcing::PostgresOutbox;
        use passages_store::PostgresPassageStore;

        Self {
            store: Arc::new(
                PostgresPassageStore::new(pool.clone())
                    .enqueuing(clock.clone(), passages_messaging::message_for),
            ),
            outbox: Arc::new(PostgresOutbox::new(pool, publisher, clock)),
        }
    }
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let passages = PassageService::new(ports.store.clone(), context.clock.clone());
    let unlink = UnlinkOnDiscard::new(passages.clone());
    let sweep = DeleteOnProjectDeleted::new(
        passages.clone(),
        context.publisher.clone(),
        context.clock.clone(),
    );

    Wired::serving(
        passages_rest::router(passages.clone())
            .merge(passages_sync::router(LivePassages::new(passages))),
    )
    .listening(vec![Arc::new(sweep), Arc::new(unlink)])
}

pub const NAME: &str = "passages";

#[cfg(feature = "postgres")]
pub async fn lay_out(pool: &sqlx::PgPool) -> Result<(), wiring::Unprepared> {
    wiring::database::lay_out(NAME, pool, eventsourcing::migrations()).await?;
    wiring::database::lay_out(NAME, pool, passages_store::migrations()).await
}
