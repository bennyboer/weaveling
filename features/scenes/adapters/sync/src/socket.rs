use std::sync::Arc;

use axum::Router;
use axum::extract::ws::{Message as Frame, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use scenes_core::SceneId;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tracing::{debug, warn};

use crate::connection::Connection;
use crate::live_scenes::{LiveScenes, Presence};
use crate::peer::Peer;

pub fn router(live: LiveScenes) -> Router {
    Router::new()
        .route("/sync", get(attach_many))
        .route("/sync/{scene}", get(attach))
        .with_state(live)
}

async fn attach(
    upgrade: WebSocketUpgrade,
    Path(scene): Path<String>,
    State(live): State<LiveScenes>,
) -> Response {
    let Ok(id) = scene.parse::<SceneId>() else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    match live.join(id).await {
        Ok(joined) => upgrade.on_upgrade(move |socket| stay(socket, joined, live)),
        Err(problem) => {
            debug!(%scene, %problem, "refusing a socket");
            StatusCode::NOT_FOUND.into_response()
        }
    }
}

async fn attach_many(upgrade: WebSocketUpgrade, State(live): State<LiveScenes>) -> Response {
    upgrade.on_upgrade(move |socket| stay_with_many(socket, live))
}

async fn write_out(
    mut sink: SplitSink<WebSocket, Frame>,
    mut outgoing: UnboundedReceiver<Vec<u8>>,
) {
    while let Some(frame) = outgoing.recv().await {
        if sink.send(Frame::Binary(frame.into())).await.is_err() {
            break;
        }
    }
}

async fn stay(socket: WebSocket, scene: Presence, live: LiveScenes) {
    let (sink, mut stream) = socket.split();
    let (to_peer, outgoing) = mpsc::unbounded_channel();
    let writing = tokio::spawn(write_out(sink, outgoing));
    let peer = Peer::arrive(live.next_peer(), to_peer, Arc::new(|frame| frame), &scene);
    let id = scene.id();

    debug!(peer = peer.id(), scene = %id, "a peer joined");
    peer.deliver(&scene.greet(), &scene);

    while let Some(Ok(frame)) = stream.next().await {
        let Frame::Binary(bytes) = frame else {
            continue;
        };

        match scene.react_to(&bytes) {
            Ok(reaction) => {
                peer.deliver(&reaction, &scene);

                if let Some(update) = reaction.to_store
                    && let Err(problem) = scene.persist(&update).await
                {
                    warn!(peer = peer.id(), %problem, "an edit reached the scene but not the store");
                }
            }
            Err(problem) => warn!(peer = peer.id(), %problem, "ignoring a frame"),
        }
    }

    debug!(peer = peer.id(), scene = %id, "a peer left");
    drop(peer);
    writing.abort();
}

async fn stay_with_many(socket: WebSocket, live: LiveScenes) {
    let (sink, mut stream) = socket.split();
    let (to_client, outgoing) = mpsc::unbounded_channel();
    let writing = tokio::spawn(write_out(sink, outgoing));
    let mut connection = Connection::new(live, to_client);

    while let Some(Ok(frame)) = stream.next().await {
        if let Frame::Binary(bytes) = frame {
            connection.hear(&bytes).await;
        }
    }

    drop(connection);
    writing.abort();
}
