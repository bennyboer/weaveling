use std::sync::Arc;

use clock::SystemClock;
use eventsourcing::Cadence;
use tokio::net::TcpListener;
use weaveling_service_api::{Adapters, Relays, app};

#[cfg(feature = "postgres")]
async fn adapters() -> Adapters {
    use weaveling_service_api::Databases;

    let server = std::env::var("DATABASE_URL").expect(
        "DATABASE_URL should name a PostgreSQL server when built with the postgres feature",
    );
    let databases = Databases::ready(&server)
        .await
        .expect("the databases should be reachable and migratable");

    Adapters::postgres(Arc::new(SystemClock), &databases)
}

#[cfg(not(feature = "postgres"))]
async fn adapters() -> Adapters {
    Adapters::in_memory(Arc::new(SystemClock))
}

async fn serving() -> (axum::Router, Relays) {
    let adapters = adapters().await;
    let outboxes = adapters.outboxes();
    let routes = app(adapters);

    (routes, Relays::started(outboxes, Cadence::default()))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

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
