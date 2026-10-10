use std::sync::Arc;

use clock::SystemClock;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;
use weaveling_service_api::{Adapters, Backend, Flakiness, Relays, WEAVELING_FLAKY, app};

const LOG_LEVEL_UNLESS_TOLD: &str = "info";

async fn serving() -> (axum::Router, Relays) {
    let backend = Backend::from_environment().unwrap_or_else(|why| panic!("{why}"));
    tracing::info!("keeping the work {backend}");
    let storage = backend.storage().unwrap_or_else(|why| panic!("{why}"));
    let mut adapters = Adapters::assembled(storage, Arc::new(SystemClock))
        .await
        .unwrap_or_else(|why| panic!("{why}"));
    if let Some(flakiness) = Flakiness::from_environment().unwrap_or_else(|why| panic!("{why}")) {
        tracing::warn!("{WEAVELING_FLAKY} is set: listeners refuse messages on purpose");
        adapters = adapters.refusing_on_purpose(flakiness);
    }
    let outboxes = adapters.outboxes();
    let consuming = adapters.consuming();
    let routes = app(adapters);

    (
        routes,
        Relays::started(outboxes, consuming, backend.cadence()),
    )
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(LOG_LEVEL_UNLESS_TOLD)),
        )
        .init();

    let (routes, relays) = serving().await;

    let listener = TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("should bind the listener");
    tracing::info!(
        "listening on http://{}",
        listener.local_addr().expect("should have a local address")
    );

    axum::serve(listener, routes)
        .with_graceful_shutdown(interrupted())
        .await
        .expect("should serve");

    stop(relays).await;
}

async fn stop(relays: Relays) {
    relays.stop().await;
    tracing::info!("the outbox relays have stopped");
}

async fn interrupted() {
    tokio::signal::ctrl_c()
        .await
        .expect("should listen for ctrl-c");

    tracing::info!("shutting down");
}
