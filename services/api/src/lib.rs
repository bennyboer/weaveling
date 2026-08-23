use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use clock::Clock;
use passages_core::{PassageService, PassageStore};
use passages_sync::LivePassages;
use projects_core::{ProjectService, ProjectStore};
use tower_http::trace::TraceLayer;

pub fn app(
    projects: Arc<dyn ProjectStore>,
    passages: Arc<dyn PassageStore>,
    clock: Arc<dyn Clock>,
) -> Router {
    let passages = PassageService::new(passages, clock.clone());

    let api = Router::new()
        .route("/health", get(health))
        .merge(projects_rest::router(ProjectService::new(projects, clock)))
        .merge(passages_rest::router(passages.clone()))
        .merge(passages_sync::router(LivePassages::new(passages)));

    Router::new()
        .nest("/api", api)
        .layer(TraceLayer::new_for_http())
}

async fn health() -> &'static str {
    "ok"
}
