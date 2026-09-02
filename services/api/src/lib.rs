use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use boards_core::{BoardCatalog, BoardEvent};
use clock::Clock;
use eventsourcing::EventStore;
use messaging::InProcessDispatcher;
use passages_core::PassageStore;
use pieces_core::{PieceCatalog, PieceEvent};
use projects_core::ProjectStore;
use tower_http::trace::TraceLayer;

pub fn app(
    projects: Arc<dyn ProjectStore>,
    passages: Arc<dyn PassageStore>,
    pieces_events: Arc<dyn EventStore<PieceEvent>>,
    pieces_catalog: Arc<dyn PieceCatalog>,
    boards_events: Arc<dyn EventStore<BoardEvent>>,
    boards_catalog: Arc<dyn BoardCatalog>,
    clock: Arc<dyn Clock>,
) -> Router {
    let dispatcher = Arc::new(InProcessDispatcher::new());

    let projects = projects_wiring::wire(projects, clock.clone());
    let pieces = pieces_wiring::wire(
        pieces_events,
        pieces_catalog,
        dispatcher.clone(),
        clock.clone(),
    );
    let boards = boards_wiring::wire(
        boards_events,
        boards_catalog,
        dispatcher.clone(),
        clock.clone(),
    );
    let passages = passages_wiring::wire(passages, clock);

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
