#[cfg(feature = "postgres")]
mod databases;
#[cfg(feature = "postgres")]
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
#[cfg(feature = "postgres")]
pub use relays::Relays;
#[cfg(feature = "postgres")]
pub use wiring::Unprepared;

pub struct Adapters {
    pub clock: Arc<dyn Clock>,
    pub dispatcher: Arc<InProcessDispatcher>,
    pub projects: projects_wiring::Ports,
    pub passages: passages_wiring::Ports,
    pub pieces: pieces_wiring::Ports,
    pub boards: boards_wiring::Ports,
    pub outline: outline_wiring::Ports,
}

impl Adapters {
    pub fn in_memory(clock: Arc<dyn Clock>) -> Self {
        let dispatcher = Arc::new(InProcessDispatcher::new());

        Self {
            clock,
            projects: projects_wiring::Ports::in_memory(),
            passages: passages_wiring::Ports::in_memory(),
            pieces: pieces_wiring::Ports::in_memory(dispatcher.clone()),
            boards: boards_wiring::Ports::in_memory(dispatcher.clone()),
            outline: outline_wiring::Ports::in_memory(dispatcher.clone()),
            dispatcher,
        }
    }

    #[cfg(feature = "postgres")]
    pub fn postgres(clock: Arc<dyn Clock>, databases: &Databases) -> Self {
        Self {
            clock,
            dispatcher: Arc::new(InProcessDispatcher::new()),
            projects: projects_wiring::Ports::postgres(databases.projects.clone()),
            passages: passages_wiring::Ports::postgres(databases.passages.clone()),
            pieces: pieces_wiring::Ports::postgres(databases.pieces.clone()),
            boards: boards_wiring::Ports::postgres(databases.boards.clone()),
            outline: outline_wiring::Ports::postgres(databases.outline.clone()),
        }
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
        pieces_wiring::wire(&adapters.pieces, &context),
        boards_wiring::wire(&adapters.boards, &context),
        outline_wiring::wire(&adapters.outline, &context),
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
