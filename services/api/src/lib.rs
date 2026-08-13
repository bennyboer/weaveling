use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use clock::Clock;
use projects_core::{ProjectService, ProjectStore};
use tower_http::trace::TraceLayer;

pub fn app(projects: Arc<dyn ProjectStore>, clock: Arc<dyn Clock>) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .merge(projects_rest::router(ProjectService::new(projects, clock)));

    Router::new()
        .nest("/api", api)
        .layer(TraceLayer::new_for_http())
}

async fn health() -> &'static str {
    "ok"
}
