use axum::extract::State;
use axum::extract::ws::{Message as Frame, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing;
use axum::{Extension, Router};
use ending::Ends;
use eventsourcing::Agent;
use tokio::sync::mpsc;

use crate::connection::Connection;
use crate::live_scenes::LiveScenes;

pub fn router(live: LiveScenes) -> Router {
    Router::new()
        .route("/scenes/sync", routing::get(attach))
        .with_state(live)
}

async fn attach(
    upgrade: WebSocketUpgrade,
    State(live): State<LiveScenes>,
    Extension(agent): Extension<Agent>,
    ends: Ends,
) -> Response {
    upgrade.on_upgrade(move |socket| stay(socket, live, agent, ends))
}

async fn stay(mut socket: WebSocket, live: LiveScenes, agent: Agent, ends: Ends) {
    let (to_client, mut outgoing) = mpsc::unbounded_channel();
    let mut connection = Connection::new(live, agent, to_client);
    let ended = ends.reached();
    tokio::pin!(ended);

    loop {
        tokio::select! {
            end = &mut ended => {
                _ = socket.send(Frame::Close(Some(end.frame()))).await;
                break;
            }
            Some(frame) = outgoing.recv() => {
                if socket.send(Frame::Binary(frame.into())).await.is_err() {
                    break;
                }
            }
            heard = socket.recv() => match heard {
                Some(Ok(Frame::Binary(bytes))) => connection.hear(&bytes).await,
                Some(Ok(_)) => {}
                Some(Err(_)) | None => break,
            },
            scene = connection.access_changed() => connection.ask_again(scene).await,
        }
    }
}
