use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::extract::ws::{Message as Frame, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use passages_core::PassageId;
use tokio::sync::mpsc::{self, UnboundedSender};
use tracing::{debug, warn};

use crate::live_passages::{LivePassage, LivePassages, Overheard, PeerId};
use crate::protocol::Message;
use crate::room::Reaction;

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

    match live.hydrate(id).await {
        Ok(passage) => upgrade.on_upgrade(move |socket| stay(socket, passage, live, id)),
        Err(problem) => {
            debug!(%passage, %problem, "refusing a socket");
            StatusCode::NOT_FOUND.into_response()
        }
    }
}

fn dispatch(
    reaction: &Reaction,
    me: PeerId,
    to_peer: &UnboundedSender<Vec<u8>>,
    passage: &LivePassage,
) {
    if let Some(reply) = &reaction.to_sender {
        let _ = to_peer.send(reply.encode());
    }
    if let Some(shared) = &reaction.to_others {
        let _ = passage.traffic.send(Overheard {
            from: me,
            frame: shared.encode(),
        });
    }
}

async fn stay(socket: WebSocket, passage: Arc<LivePassage>, live: LivePassages, id: PassageId) {
    static NEXT: AtomicUsize = AtomicUsize::new(1);
    let me = NEXT.fetch_add(1, Ordering::Relaxed);

    let (mut sink, mut stream) = socket.split();
    let (to_peer, mut outgoing) = mpsc::unbounded_channel::<Vec<u8>>();
    let mut traffic = passage.traffic.subscribe();

    let writing = tokio::spawn(async move {
        while let Some(frame) = outgoing.recv().await {
            if sink.send(Frame::Binary(frame.into())).await.is_err() {
                break;
            }
        }
    });

    let overhearing = tokio::spawn({
        let to_peer = to_peer.clone();
        async move {
            while let Ok(overheard) = traffic.recv().await {
                if overheard.from != me && to_peer.send(overheard.frame).is_err() {
                    break;
                }
            }
        }
    });

    debug!(peer = me, passage = %id, "a peer joined");
    dispatch(&passage.room.greet(), me, &to_peer, &passage);

    while let Some(Ok(frame)) = stream.next().await {
        let Frame::Binary(bytes) = frame else {
            continue;
        };

        let reaction = match Message::decode(&bytes) {
            Ok(message) => match passage.room.receive(message) {
                Ok(reaction) => reaction,
                Err(problem) => {
                    warn!(peer = me, %problem, "ignoring a frame the room refused");
                    continue;
                }
            },
            Err(problem) => {
                warn!(peer = me, %problem, "ignoring a frame we could not read");
                continue;
            }
        };

        dispatch(&reaction, me, &to_peer, &passage);

        if let Some(update) = reaction.to_store
            && let Err(problem) = live.persist(id, &update).await
        {
            warn!(peer = me, %problem, "an edit reached the room but not the store");
        }
    }

    debug!(peer = me, passage = %id, "a peer left");
    overhearing.abort();
    drop(to_peer);
    let _ = writing.await;
}
