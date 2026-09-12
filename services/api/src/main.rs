use std::sync::Arc;

use clock::SystemClock;
use tokio::net::TcpListener;
use weaveling_service_api::{Adapters, app};

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

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = app(adapters().await);

    let listener = TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("should bind the listener");
    tracing::info!(
        "listening on http://{}",
        listener.local_addr().expect("should have a local address")
    );

    axum::serve(listener, app).await.expect("should serve");
}
