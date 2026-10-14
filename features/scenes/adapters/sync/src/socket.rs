use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message as Frame, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::get;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc::{self, UnboundedReceiver};

use crate::connection::Connection;
use crate::live_scenes::LiveScenes;

pub fn router(live: LiveScenes) -> Router {
    Router::new().route("/sync", get(attach)).with_state(live)
}

async fn attach(upgrade: WebSocketUpgrade, State(live): State<LiveScenes>) -> Response {
    upgrade.on_upgrade(move |socket| stay(socket, live))
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

async fn stay(socket: WebSocket, live: LiveScenes) {
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
