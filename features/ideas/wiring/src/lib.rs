use std::sync::Arc;

use eventsourcing::{EventStore, InMemoryEventStore, InMemoryOutbox, Outbox};
use ideas_catalog::InMemoryIdeaCatalog;
use ideas_core::{IdeaCatalog, IdeaEvent, IdeaService};
use ideas_messaging::{DiscardOnProjectDeleted, IdeaCatalogProjector};
use wiring::{Context, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<IdeaEvent>>,
    pub catalog: Arc<dyn IdeaCatalog>,
    pub outbox: Arc<dyn Outbox>,
}

impl Ports {
    pub fn in_memory(
        publisher: Arc<dyn messaging::Publisher>,
        clock: Arc<dyn clock::Clock>,
    ) -> Self {
        let outbox = Arc::new(InMemoryOutbox::new(publisher, clock));

        Self {
            events: Arc::new(InMemoryEventStore::enqueuing_to(
                outbox.clone(),
                ideas_messaging::message_for,
            )),
            catalog: Arc::new(InMemoryIdeaCatalog::new()),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    pub fn postgres(
        pool: sqlx::PgPool,
        publisher: Arc<dyn messaging::Publisher>,
        clock: Arc<dyn clock::Clock>,
    ) -> Self {
        use eventsourcing::{PostgresEventStore, PostgresOutbox};

        Self {
            events: Arc::new(PostgresEventStore::new(
                pool.clone(),
                ideas_store::codec(),
                ideas_messaging::message_for,
            )),
            catalog: Arc::new(ideas_catalog::PostgresIdeaCatalog::new(pool.clone())),
            outbox: Arc::new(PostgresOutbox::new(pool, publisher, clock)),
        }
    }
}

pub fn service(ports: &Ports, context: &Context) -> IdeaService {
    IdeaService::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let ideas = service(ports, context);
    let projector = IdeaCatalogProjector::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    );
    let sweep = DiscardOnProjectDeleted::new(
        ideas.clone(),
        ports.catalog.clone(),
        context.publisher.clone(),
        context.clock.clone(),
    );

    Wired::serving(ideas_rest::router(ideas)).listening(vec![Arc::new(projector), Arc::new(sweep)])
}

pub const NAME: &str = "ideas";

#[cfg(feature = "postgres")]
pub async fn lay_out(pool: &sqlx::PgPool) -> Result<(), wiring::Unprepared> {
    wiring::database::lay_out(NAME, pool, eventsourcing::migrations()).await?;
    wiring::database::lay_out(NAME, pool, ideas_catalog::migrations()).await
}
