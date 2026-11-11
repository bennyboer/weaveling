use std::fmt::Display;
use std::process;
use std::sync::Arc;
use std::time::Duration;

use axum::Extension;
use clock::SystemClock;
use ending::Shutdown;
use tokio::net::TcpListener;
#[cfg(unix)]
use tokio::signal::unix::SignalKind;
use tracing_subscriber::EnvFilter;
use weaveling_service_api::{Accounts, Adapters, Chores, Mailing, Relays, Settings, Settling};

const LOG_LEVEL_UNLESS_TOLD: &str = "info";
const SOCKETS_GIVEN_TO_CLOSE: Duration = Duration::from_secs(60);

async fn serving(settings: &Settings) -> (axum::Router, Relays, Vec<Arc<dyn Settling>>, Chores) {
    let backend = or_stop(settings.backend());
    let accounts = or_stop(settings.accounts());
    let mailing = or_stop(or_stop(settings.mailing()).ready_for(accounts));
    let sign_up = or_stop(settings.sign_up());
    let totp_key = or_stop(settings.totp_key());
    let key_ceremonies = or_stop(settings.key_ceremonies());
    match accounts {
        Accounts::On => {
            tracing::info!("keeping the work {backend}, {accounts}, {sign_up}, {mailing}");
            if mailing == Mailing::Logged {
                tracing::warn!(
                    "every mail is written to the log, codes and invitation links in it: whoever reads the log can sign in as anyone; only ever for development"
                );
            }
            if totp_key.is_none() {
                tracing::warn!(
                    "no TOTP key is set, so two-step sign-in cannot be set up; see weaveling.example.toml"
                );
            }
            if key_ceremonies.is_none() {
                tracing::warn!(
                    "no public URL is set, so no key or passkey can be added and nobody invited; see weaveling.example.toml"
                );
            }
        }
        Accounts::Off => tracing::info!("keeping the work {backend}, {accounts}, {mailing}"),
    }
    let storage = or_stop(backend.storage());
    let mut adapters = Adapters::assembled(
        storage,
        accounts,
        mailing.mailer(),
        settings.public_url(),
        Arc::new(SystemClock),
    )
    .await
    .unwrap_or_else(|why| panic!("{why}"));
    adapters = adapters.signing_up(sign_up);
    if let Some(key) = totp_key {
        adapters = adapters.sealing_totp_secrets(key);
    }
    if let Some(ceremonies) = key_ceremonies {
        adapters = adapters.holding_key_ceremonies(ceremonies);
    }
    if let Some(flakiness) = or_stop(settings.flakiness()) {
        tracing::warn!("flakiness is set: listeners refuse messages on purpose");
        adapters = adapters.refusing_on_purpose(flakiness);
    }
    if let Some(code) = adapters
        .draw_operator_code()
        .await
        .unwrap_or_else(|why| panic!("{why}"))
    {
        tracing::warn!(
            "there is no operator yet: open /setup and enter the code {}",
            code.as_str()
        );
    }
    let outboxes = adapters.outboxes();
    let consuming = adapters.consuming();
    let settling = adapters.settling();
    let chores = adapters.chores();
    let mut routes = weaveling_service_api::app(adapters);
    if let Some(client) = settings.client() {
        tracing::info!("handing out the client built in {}", client.dir.display());
        routes = weaveling_service_api::serving_the_client(routes, client);
    }
    let routes = weaveling_service_api::answering_only_its_own(
        routes,
        settings.public_url().as_deref(),
        or_stop(settings.hosts()),
    );

    (
        routes,
        Relays::started(outboxes, consuming, backend.cadence()),
        settling,
        Chores::started(chores),
    )
}

fn or_stop<T>(settled: Result<T, impl Display>) -> T {
    settled.unwrap_or_else(|why| {
        eprintln!("{why}");
        process::exit(2)
    })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(LOG_LEVEL_UNLESS_TOLD)),
        )
        .init();

    let settings = or_stop(Settings::of_this_process());
    let listen = or_stop(settings.listen());
    let (routes, relays, settling, chores) = serving(&settings).await;
    let shutdown = Shutdown::new();
    let routes = routes.layer(Extension(shutdown.clone()));

    let listener = or_stop(
        TcpListener::bind(listen)
            .await
            .map_err(|why| format!("cannot listen at {listen}: {why}")),
    );
    tracing::info!(
        "listening on http://{}",
        listener.local_addr().expect("should have a local address")
    );

    axum::serve(listener, routes)
        .with_graceful_shutdown({
            let shutdown = shutdown.clone();
            async move {
                interrupted().await;
                shutdown.close();
            }
        })
        .await
        .expect("should serve");

    if tokio::time::timeout(SOCKETS_GIVEN_TO_CLOSE, shutdown.drained())
        .await
        .is_err()
    {
        tracing::warn!("some sockets did not close in time, so what they were writing may be lost");
    }
    tracing::info!("the sockets have closed");
    stop(settling, relays, chores).await;
}

async fn stop(settling: Vec<Arc<dyn Settling>>, relays: Relays, chores: Chores) {
    chores.stop().await;
    tracing::info!("the chores have stopped");

    for settled in settling {
        settled.settle().await;
    }
    tracing::info!("what was still in memory is settled");

    relays.stop().await;
    tracing::info!("the outbox relays have stopped");
}

async fn interrupted() {
    tokio::select! {
        () = ctrl_c() => {}
        () = terminated() => {}
    }

    tracing::info!("shutting down");
}

async fn ctrl_c() {
    tokio::signal::ctrl_c()
        .await
        .expect("should listen for ctrl-c");
}

#[cfg(unix)]
async fn terminated() {
    tokio::signal::unix::signal(SignalKind::terminate())
        .expect("should listen for a request to terminate")
        .recv()
        .await;
}

#[cfg(not(unix))]
async fn terminated() {
    std::future::pending::<()>().await;
}
