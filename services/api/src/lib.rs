use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use clock::Clock;
use messaging::InProcessDispatcher;
use tower_http::trace::TraceLayer;

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
    let clock = adapters.clock;

    let projects = projects_wiring::wire(adapters.projects, clock.clone());
    let passages = passages_wiring::wire(adapters.passages, clock.clone());
    let pieces = pieces_wiring::wire(adapters.pieces, dispatcher.clone(), clock.clone());
    let boards = boards_wiring::wire(adapters.boards, dispatcher.clone(), clock);

    // Register msg listeners
    for listener in pieces.listeners.into_iter().chain(boards.listeners) {
        dispatcher.listen(listener);
    }

    // Register HTTP routing
    let api = Router::new()
        .route("/health", get(health))
        .merge(projects.routes)
        .merge(passages.routes)
        .merge(pieces.routes)
        .merge(boards.routes);

    Router::new()
        .nest("/api", api)
        .layer(TraceLayer::new_for_http())
}

async fn health() -> &'static str {
    "ok"
}
