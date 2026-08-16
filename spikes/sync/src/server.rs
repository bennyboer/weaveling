use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use axum::Router;
use axum::extract::ws::{Message as Frame, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::Response;
use axum::routing::get;
use crdt_spike::projection::plain_text;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, warn};

use crate::protocol::Message;
use crate::room::{Room, RoomError};

type Peer = usize;

const BACKLOG: usize = 256;

struct Occupied {
    room: Room,
    traffic: broadcast::Sender<(Peer, Vec<u8>)>,
}

#[derive(Clone, Default)]
pub struct Rooms(Arc<RwLock<HashMap<String, Arc<Occupied>>>>);

impl Rooms {
    fn enter(&self, name: &str) -> Arc<Occupied> {
        if let Some(found) = self.0.read().expect("rooms lock").get(name) {
            return found.clone();
        }

        self.0
            .write()
            .expect("rooms lock")
            .entry(name.to_owned())
            .or_insert_with(|| {
                Arc::new(Occupied {
                    room: Room::new(),
                    traffic: broadcast::channel(BACKLOG).0,
                })
            })
            .clone()
    }

    pub fn prose(&self, name: &str) -> Option<String> {
        self.0
            .read()
            .expect("rooms lock")
            .get(name)
            .map(|occupied| plain_text(occupied.room.doc()))
    }
}

pub fn app(rooms: Rooms) -> Router {
    Router::new()
        .route("/sync/{room}", get(attach))
        .route("/rooms/{room}", get(look))
        .with_state(rooms)
}

async fn look(Path(name): Path<String>, State(rooms): State<Rooms>) -> String {
    rooms.prose(&name).unwrap_or_default()
}

async fn attach(
    upgrade: WebSocketUpgrade,
    Path(name): Path<String>,
    State(rooms): State<Rooms>,
) -> Response {
    let occupied = rooms.enter(&name);

    upgrade.on_upgrade(move |socket| stay(socket, occupied))
}

async fn stay(socket: WebSocket, occupied: Arc<Occupied>) {
    static NEXT: AtomicUsize = AtomicUsize::new(1);
    let me = NEXT.fetch_add(1, Ordering::Relaxed);

    let (mut sink, mut stream) = socket.split();
    let (out, mut outbox) = mpsc::unbounded_channel::<Vec<u8>>();
    let mut traffic = occupied.traffic.subscribe();

    let writing = tokio::spawn(async move {
        while let Some(frame) = outbox.recv().await {
            if sink.send(Frame::Binary(frame.into())).await.is_err() {
                break;
            }
        }
    });

    let overhearing = tokio::spawn({
        let out = out.clone();
        async move {
            while let Ok((from, frame)) = traffic.recv().await {
                if from != me && out.send(frame).is_err() {
                    break;
                }
            }
        }
    });

    let _ = out.send(occupied.room.greet().encode());
    let _ = occupied.traffic.send((me, Message::WhoIsHere.encode()));
    debug!(peer = me, "a peer joined");

    while let Some(Ok(frame)) = stream.next().await {
        let Frame::Binary(bytes) = frame else {
            continue;
        };

        match Message::decode(&bytes)
            .map_err(RoomError::from)
            .and_then(|message| occupied.room.receive(message))
        {
            Ok(reaction) => {
                if let Some(reply) = reaction.to_sender {
                    let _ = out.send(reply.encode());
                }
                if let Some(shared) = reaction.to_others {
                    let _ = occupied.traffic.send((me, shared.encode()));
                }
            }
            Err(problem) => warn!(peer = me, %problem, "ignoring a frame"),
        }
    }

    debug!(peer = me, "a peer left");
    overhearing.abort();
    drop(out);
    let _ = writing.await;
}
