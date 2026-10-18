use std::sync::Arc;

use eventsourcing::{Event, EventStore, InMemoryEventStore, MessageMapping};
use outbox::{InMemoryOutbox, Outbox};

use crate::Context;

#[cfg(any(feature = "postgres", feature = "sqlite"))]
use eventsourcing::Codec;
#[cfg(feature = "postgres")]
use eventsourcing::PostgresEventStore;
#[cfg(feature = "sqlite")]
use eventsourcing::SqliteEventStore;
#[cfg(feature = "postgres")]
use outbox::PostgresOutbox;
#[cfg(feature = "sqlite")]
use outbox::SqliteOutbox;

pub struct EventSourced<E> {
    pub events: Arc<dyn EventStore<E>>,
    pub outbox: Arc<dyn Outbox>,
}

impl<E> EventSourced<E>
where
    E: Event + Clone + Send + Sync + 'static,
{
    pub fn in_memory(message_for: MessageMapping<E>, context: &Context) -> Self {
        let outbox = Arc::new(InMemoryOutbox::new(
            context.publisher.clone(),
            context.clock.clone(),
        ));

        Self {
            events: Arc::new(InMemoryEventStore::enqueuing_to(
                outbox.clone(),
                message_for,
            )),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    pub fn on_postgres(
        pool: &sqlx::PgPool,
        codec: Codec<E>,
        message_for: MessageMapping<E>,
        context: &Context,
    ) -> Self {
        Self {
            events: Arc::new(PostgresEventStore::new(pool.clone(), codec, message_for)),
            outbox: Arc::new(PostgresOutbox::new(
                pool.clone(),
                context.publisher.clone(),
                context.clock.clone(),
            )),
        }
    }

    #[cfg(feature = "sqlite")]
    pub fn on_sqlite(
        pool: &sqlx::SqlitePool,
        codec: Codec<E>,
        message_for: MessageMapping<E>,
        context: &Context,
    ) -> Self {
        Self {
            events: Arc::new(SqliteEventStore::new(pool.clone(), codec, message_for)),
            outbox: Arc::new(SqliteOutbox::new(
                pool.clone(),
                context.publisher.clone(),
                context.clock.clone(),
            )),
        }
    }
}
