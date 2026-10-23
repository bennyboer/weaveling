use std::sync::Arc;

use clock::SystemClock;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;
use weaveling_service_api::{
    Accounts, Adapters, Backend, Flakiness, Mailing, Relays, Settling, SignUp, WEAVELING_FLAKY,
};

const LOG_LEVEL_UNLESS_TOLD: &str = "info";

async fn serving() -> (axum::Router, Relays, Vec<Arc<dyn Settling>>) {
    let backend = Backend::from_environment().unwrap_or_else(|why| panic!("{why}"));
    let accounts = Accounts::from_environment().unwrap_or_else(|why| panic!("{why}"));
    let mailing = Mailing::from_environment();
    let sign_up = SignUp::from_environment().unwrap_or_else(|why| panic!("{why}"));
    match accounts {
        Accounts::On => {
            tracing::info!("keeping the work {backend}, {accounts}, {sign_up}, {mailing}")
        }
        Accounts::Off => tracing::info!("keeping the work {backend}, {accounts}, {mailing}"),
    }
    let storage = backend.storage().unwrap_or_else(|why| panic!("{why}"));
    let mut adapters =
        Adapters::assembled(storage, accounts, mailing.mailer(), Arc::new(SystemClock))
            .await
            .unwrap_or_else(|why| panic!("{why}"));
    adapters = adapters.signing_up(sign_up);
    if let Some(flakiness) = Flakiness::from_environment().unwrap_or_else(|why| panic!("{why}")) {
        tracing::warn!("{WEAVELING_FLAKY} is set: listeners refuse messages on purpose");
        adapters = adapters.refusing_on_purpose(flakiness);
    }
    if let Some(code) = adapters
        .draw_operator_code()
        .await
        .unwrap_or_else(|why| panic!("{why}"))
    {
        tracing::warn!(
            "there is no operator yet: sign in, open /instance and enter the code {}",
            code.as_str()
        );
    }
    let outboxes = adapters.outboxes();
    let consuming = adapters.consuming();
    let settling = adapters.settling();
    let routes = weaveling_service_api::app(adapters);

    (
        routes,
        Relays::started(outboxes, consuming, backend.cadence()),
        settling,
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

    let (routes, relays, settling) = serving().await;

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

    stop(settling, relays).await;
}

async fn stop(settling: Vec<Arc<dyn Settling>>, relays: Relays) {
    for settled in settling {
        settled.settle().await;
    }
    tracing::info!("what was still in memory is settled");

    relays.stop().await;
    tracing::info!("the outbox relays have stopped");
}

async fn interrupted() {
    tokio::signal::ctrl_c()
        .await
        .expect("should listen for ctrl-c");

    tracing::info!("shutting down");
}
