use axum::Router;
use axum::extract::ws::{Message as Frame, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use futures_util::StreamExt;
use scenes_core::SceneId;
use tracing::{debug, warn};

use crate::live_scenes::{LiveScenes, Presence};
use crate::peer::Peer;

pub fn router(live: LiveScenes) -> Router {
    Router::new()
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

async fn stay(socket: WebSocket, scene: Presence, live: LiveScenes) {
    let (sink, mut stream) = socket.split();
    let peer = Peer::arrive(live.next_peer(), sink, &scene);
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
}
