use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tokio::net::TcpListener;
use weaveling_spike_sync::server::{Rooms, app};

fn interop_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("interop")
}

fn ready() -> bool {
    interop_dir().join("node_modules/y-websocket").exists()
}

fn exchange_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("weaveling-sync-interop");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("exchange directory should be creatable");

    dir
}

fn text_at(exchange: &Path, name: &str) -> String {
    fs::read_to_string(exchange.join(name)).expect("the clients should have written this file")
}

#[tokio::test]
async fn real_y_websocket_clients_sync_through_our_server() {
    if !ready() {
        eprintln!("skipping: run `npm install` in spikes/sync/interop first");
        return;
    }

    let rooms = Rooms::default();
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("an ephemeral port should be available");
    let port = listener
        .local_addr()
        .expect("the port should be known")
        .port();
    tokio::spawn(axum::serve(listener, app(rooms.clone())).into_future());

    let exchange = exchange_dir();
    let outcome = tokio::task::spawn_blocking({
        let exchange = exchange.clone();
        move || {
            Command::new("node")
                .arg(interop_dir().join("two-clients.mjs"))
                .arg(port.to_string())
                .arg(&exchange)
                .output()
                .expect("node should be runnable")
        }
    })
    .await
    .expect("the client task should finish");

    assert!(
        outcome.status.success(),
        "the y-websocket clients failed:\n{}",
        String::from_utf8_lossy(&outcome.stderr)
    );

    assert!(
        text_at(&exchange, "bo-caught-up.txt").contains("grey morning light"),
        "a client joining after the edit must be served by the room itself"
    );

    let converged = text_at(&exchange, "converged.txt");
    assert!(converged.contains("Ada wrote second."));
    assert!(converged.contains("Bo wrote second."));

    assert_eq!(
        text_at(&exchange, "awareness.txt"),
        "Ada,Bo",
        "awareness must reach the other peer, though the server never decodes it"
    );

    assert_eq!(
        text_at(&exchange, "server-view.txt"),
        converged,
        "the server's own projection must match what the clients see"
    );

    assert_eq!(
        rooms.prose("chapter-one").as_deref(),
        Some(converged.as_str()),
        "the room should still hold the document after everyone disconnects"
    );

    println!("\n--- the server's view of the room ---");
    for line in converged.lines() {
        println!("  |{line}");
    }
}
