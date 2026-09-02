use std::sync::Arc;

use boards_catalog::InMemoryBoardCatalog;
use clock::SystemClock;
use eventsourcing::InMemoryEventStore;
use passages_store::InMemoryPassageStore;
use pieces_catalog::InMemoryPieceCatalog;
use projects_store::InMemoryProjectStore;
use tokio::net::TcpListener;
use weaveling_service_api::app;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = app(
        Arc::new(InMemoryProjectStore::new()),
        Arc::new(InMemoryPassageStore::new()),
        Arc::new(InMemoryEventStore::new()),
        Arc::new(InMemoryPieceCatalog::new()),
        Arc::new(InMemoryEventStore::new()),
        Arc::new(InMemoryBoardCatalog::new()),
        Arc::new(SystemClock),
    );

    let listener = TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("should bind the listener");
    tracing::info!(
        "listening on http://{}",
        listener.local_addr().expect("should have a local address")
    );

    axum::serve(listener, app).await.expect("should serve");
}
