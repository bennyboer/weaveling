use std::sync::Arc;

use clock::SystemClock;
use outbox::Cadence;
use tokio::net::TcpListener;
use weaveling_service_api::{Adapters, Relays, Storage, app};

#[cfg(feature = "postgres")]
fn storage() -> Storage {
    let server = std::env::var("DATABASE_URL").expect(
        "DATABASE_URL should name a PostgreSQL server when built with the postgres feature",
    );

    Storage::Postgres(Arc::new(wiring::ServerDatabases::on(&server)))
}

#[cfg(not(feature = "postgres"))]
fn storage() -> Storage {
    Storage::InMemory
}

async fn adapters() -> Adapters {
    Adapters::assembled(storage(), Arc::new(SystemClock))
        .await
        .expect("the databases should be reachable and migratable")
}

async fn serving() -> (axum::Router, Relays) {
    let adapters = adapters().await;
    let outboxes = adapters.outboxes();
    let consuming = adapters.consuming();
    let routes = app(adapters);

    (
        routes,
        Relays::started(outboxes, consuming, Cadence::default()),
    )
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
