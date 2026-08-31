use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use clock::Clock;
use eventsourcing::EventStore;
use messaging::InProcessDispatcher;
use passages_core::{PassageService, PassageStore};
use passages_sync::LivePassages;
use pieces_core::{PieceCatalog, PieceEvent, PieceService};
use projects_core::{ProjectService, ProjectStore};
use tower_http::trace::TraceLayer;

pub fn app(
    projects: Arc<dyn ProjectStore>,
    passages: Arc<dyn PassageStore>,
    pieces_events: Arc<dyn EventStore<PieceEvent>>,
    pieces_catalog: Arc<dyn PieceCatalog>,
    clock: Arc<dyn Clock>,
) -> Router {
    let passages = PassageService::new(passages, clock.clone());
    let dispatcher = Arc::new(InProcessDispatcher::new());

    dispatcher.listen(Arc::new(pieces_messaging::PieceCatalogProjector::new(
        pieces_events.clone(),
        pieces_catalog.clone(),
        clock.clone(),
    )));

    let api = Router::new()
        .route("/health", get(health))
        .merge(projects_rest::router(ProjectService::new(
            projects,
            clock.clone(),
        )))
        .merge(passages_rest::router(passages.clone()))
        .merge(passages_sync::router(LivePassages::new(passages)))
        .merge(pieces_rest::router(PieceService::new(
            pieces_events,
            pieces_catalog,
            Arc::new(pieces_messaging::Publishing::new(dispatcher)),
            clock,
        )));

    Router::new()
        .nest("/api", api)
        .layer(TraceLayer::new_for_http())
}

async fn health() -> &'static str {
    "ok"
}
