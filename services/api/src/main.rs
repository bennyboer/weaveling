use std::sync::Arc;

use clock::SystemClock;
use tokio::net::TcpListener;
use weaveling_service_api::{Adapters, app};

#[cfg(feature = "postgres")]
async fn serving() -> (axum::Router, Option<weaveling_service_api::Relays>) {
    use eventsourcing::Cadence;
    use weaveling_service_api::{Databases, Relays};

    let clock = Arc::new(SystemClock);
    let server = std::env::var("DATABASE_URL").expect(
        "DATABASE_URL should name a PostgreSQL server when built with the postgres feature",
    );
    let databases = Databases::ready(&server)
        .await
        .expect("the databases should be reachable and migratable");

    let adapters = Adapters::postgres(clock.clone(), &databases);
    let publisher = adapters.dispatcher.clone();
    let routes = app(adapters);
    let relays = Relays::started(&databases, publisher, clock, Cadence::default());

    (routes, Some(relays))
}

#[cfg(not(feature = "postgres"))]
async fn serving() -> (axum::Router, Option<()>) {
    (app(Adapters::in_memory(Arc::new(SystemClock))), None)
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

#[cfg(feature = "postgres")]
async fn stop(relays: Option<weaveling_service_api::Relays>) {
    if let Some(relays) = relays {
        relays.stop().await;
        tracing::info!("the outbox relays have stopped");
    }
}

#[cfg(not(feature = "postgres"))]
async fn stop(_relays: Option<()>) {}

async fn interrupted() {
    tokio::signal::ctrl_c()
        .await
        .expect("should listen for ctrl-c");

    tracing::info!("shutting down");
}
