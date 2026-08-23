use std::sync::Arc;

use axum::Router;
use axum::extract::ws::{Message as Frame, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use futures_util::StreamExt;
use passages_core::PassageId;
use tracing::{debug, warn};

use crate::live_passages::{LivePassage, LivePassages};
use crate::peer::Peer;

pub fn router(live: LivePassages) -> Router {
    Router::new()
        .route("/sync/{passage}", get(attach))
        .with_state(live)
}

async fn attach(
    upgrade: WebSocketUpgrade,
    Path(passage): Path<String>,
    State(live): State<LivePassages>,
) -> Response {
    let Ok(id) = passage.parse::<PassageId>() else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    match live.join(id).await {
        Ok(joined) => upgrade.on_upgrade(move |socket| stay(socket, joined, live)),
        Err(problem) => {
            debug!(%passage, %problem, "refusing a socket");
            StatusCode::NOT_FOUND.into_response()
        }
    }
}

async fn stay(socket: WebSocket, passage: Arc<LivePassage>, live: LivePassages) {
    let (sink, mut stream) = socket.split();
    let peer = Peer::arrive(live.next_peer(), sink, &passage);
    let id = passage.id();

    debug!(peer = peer.id(), passage = %id, "a peer joined");
    peer.deliver(&passage.greet(), &passage);

    while let Some(Ok(frame)) = stream.next().await {
        let Frame::Binary(bytes) = frame else {
            continue;
        };

        match passage.react_to(&bytes) {
            Ok(reaction) => {
                peer.deliver(&reaction, &passage);

                if let Some(update) = reaction.to_store
                    && let Err(problem) = live.persist(id, &update).await
                {
                    warn!(peer = peer.id(), %problem, "an edit reached the passage but not the store");
                }
            }
            Err(problem) => warn!(peer = peer.id(), %problem, "ignoring a frame"),
        }
    }

    debug!(peer = peer.id(), passage = %id, "a peer left");
}
