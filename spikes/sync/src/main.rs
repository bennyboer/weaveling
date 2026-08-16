use tokio::net::TcpListener;
use tracing::info;
use weaveling_spike_sync::server::{Rooms, app};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,weaveling_spike_sync=debug".into()),
        )
        .init();

    let listener = TcpListener::bind("127.0.0.1:3001")
        .await
        .expect("port 3001 should be free");

    info!("sync spike listening on ws://127.0.0.1:3001/sync/{{room}}");
    axum::serve(listener, app(Rooms::default()))
        .await
        .expect("the server should run");
}
