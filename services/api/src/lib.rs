#[cfg(feature = "postgres")]
mod databases;
mod relays;

use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use clock::Clock;
use messaging::InProcessDispatcher;
use tower_http::trace::TraceLayer;
use wiring::{Context, Wired};

#[cfg(feature = "postgres")]
pub use databases::Databases;
pub use relays::Relays;
#[cfg(feature = "postgres")]
pub use wiring::Unprepared;

pub struct Adapters {
    pub clock: Arc<dyn Clock>,
    pub dispatcher: Arc<InProcessDispatcher>,
    pub deliveries: Arc<dyn messaging::Deliveries>,
    pub projects: projects_wiring::Ports,
    pub passages: passages_wiring::Ports,
    pub ideas: ideas_wiring::Ports,
    pub boards: boards_wiring::Ports,
    pub outline: outline_wiring::Ports,
    pub appearances: appearances_wiring::Ports,
}

impl Adapters {
    pub fn in_memory(clock: Arc<dyn Clock>) -> Self {
        let deliveries = Arc::new(messaging::InMemoryDeliveries::new());
        let dispatcher = Arc::new(InProcessDispatcher::queueing_to(deliveries.clone()));

        Self {
            projects: projects_wiring::Ports::in_memory(dispatcher.clone(), clock.clone()),
            passages: passages_wiring::Ports::in_memory(dispatcher.clone(), clock.clone()),
            ideas: ideas_wiring::Ports::in_memory(dispatcher.clone(), clock.clone()),
            boards: boards_wiring::Ports::in_memory(dispatcher.clone(), clock.clone()),
            outline: outline_wiring::Ports::in_memory(dispatcher.clone(), clock.clone()),
            appearances: appearances_wiring::Ports::in_memory(),
            clock,
            dispatcher,
            deliveries,
        }
    }

    #[cfg(feature = "postgres")]
    pub fn postgres(clock: Arc<dyn Clock>, databases: &Databases) -> Self {
        let deliveries = Arc::new(messaging::PostgresDeliveries::new(
            databases.messaging.clone(),
        ));
        let dispatcher = Arc::new(InProcessDispatcher::queueing_to(deliveries.clone()));

        Self {
            projects: projects_wiring::Ports::postgres(
                databases.projects.clone(),
                dispatcher.clone(),
                clock.clone(),
            ),
            passages: passages_wiring::Ports::postgres(
                databases.passages.clone(),
                dispatcher.clone(),
                clock.clone(),
            ),
            ideas: ideas_wiring::Ports::postgres(
                databases.ideas.clone(),
                dispatcher.clone(),
                clock.clone(),
            ),
            boards: boards_wiring::Ports::postgres(
                databases.boards.clone(),
                dispatcher.clone(),
                clock.clone(),
            ),
            outline: outline_wiring::Ports::postgres(
                databases.outline.clone(),
                dispatcher.clone(),
                clock.clone(),
            ),
            appearances: appearances_wiring::Ports::postgres(databases.appearances.clone()),
            clock,
            dispatcher,
            deliveries,
        }
    }

    pub fn consuming(&self) -> messaging::DeliveryConsumer {
        messaging::DeliveryConsumer::new(
            self.dispatcher.clone(),
            self.deliveries.clone(),
            self.clock.clone(),
        )
    }

    pub fn outboxes(&self) -> Vec<Arc<dyn eventsourcing::Outbox>> {
        vec![
            self.projects.outbox.clone(),
            self.ideas.outbox.clone(),
            self.boards.outbox.clone(),
            self.outline.outbox.clone(),
            self.passages.outbox.clone(),
        ]
    }
}

pub fn app(adapters: Adapters) -> Router {
    let dispatcher = adapters.dispatcher.clone();
    let context = Context {
        clock: adapters.clock,
        publisher: dispatcher.clone(),
    };

    let features = vec![
        projects_wiring::wire(&adapters.projects, &context),
        passages_wiring::wire(&adapters.passages, &context),
        ideas_wiring::wire(&adapters.ideas, &context),
        boards_wiring::wire(&adapters.boards, &context),
        outline_wiring::wire(&adapters.outline, &context),
        appearances_wiring::wire(&adapters.appearances),
    ];

    Router::new()
        .nest("/api", assembled(features, &dispatcher))
        .layer(TraceLayer::new_for_http())
}

fn assembled(features: Vec<Wired>, dispatcher: &InProcessDispatcher) -> Router {
    let mut api = Router::new().route("/health", get(health));

    for feature in features {
        api = api.merge(feature.routes);

        for listener in feature.listeners {
            dispatcher.listen(listener);
        }
    }

    api
}

async fn health() -> &'static str {
    "ok"
}
