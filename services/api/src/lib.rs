use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use clock::Clock;
use messaging::InProcessDispatcher;
use tower_http::trace::TraceLayer;
use wiring::{Context, Wired};

pub struct Adapters {
    pub clock: Arc<dyn Clock>,
    pub projects: projects_wiring::Ports,
    pub passages: passages_wiring::Ports,
    pub pieces: pieces_wiring::Ports,
    pub boards: boards_wiring::Ports,
}

impl Adapters {
    pub fn in_memory(clock: Arc<dyn Clock>) -> Self {
        Self {
            clock,
            projects: projects_wiring::Ports::in_memory(),
            passages: passages_wiring::Ports::in_memory(),
            pieces: pieces_wiring::Ports::in_memory(),
            boards: boards_wiring::Ports::in_memory(),
        }
    }
}

pub fn app(adapters: Adapters) -> Router {
    let dispatcher = Arc::new(InProcessDispatcher::new());
    let context = Context {
        clock: adapters.clock,
        publisher: dispatcher.clone(),
    };

    let features = vec![
        projects_wiring::wire(&adapters.projects, &context),
        passages_wiring::wire(&adapters.passages, &context),
        pieces_wiring::wire(&adapters.pieces, &context),
        boards_wiring::wire(&adapters.boards, &context),
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
