use acting::SessionEnd;
use axum::extract::State;
use axum::extract::ws::{CloseFrame, Message as Frame, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing;
use axum::{Extension, Router};
use eventsourcing::Agent;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use live_contract::SESSION_ENDED;
use tokio::sync::mpsc::{self, UnboundedReceiver};

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
    session: Option<Extension<SessionEnd>>,
) -> Response {
    let session = session.map_or_else(SessionEnd::never, |Extension(session)| session);
    upgrade.on_upgrade(move |socket| stay(socket, live, agent, session))
}

async fn write_out(
    mut sink: SplitSink<WebSocket, Frame>,
    mut outgoing: UnboundedReceiver<Vec<u8>>,
    session: SessionEnd,
) {
    let session_ended = session.reached();
    tokio::pin!(session_ended);

    loop {
        tokio::select! {
            () = &mut session_ended => {
                _ = sink
                    .send(Frame::Close(Some(CloseFrame {
                        code: SESSION_ENDED,
                        reason: "session-ended".into(),
                    })))
                    .await;
                break;
            }
            frame = outgoing.recv() => {
                let Some(frame) = frame else {
                    break;
                };
                if sink.send(Frame::Binary(frame.into())).await.is_err() {
                    break;
                }
            }
        }
    }
}

async fn stay(socket: WebSocket, live: LiveScenes, agent: Agent, session: SessionEnd) {
    let (sink, mut stream) = socket.split();
    let (to_client, outgoing) = mpsc::unbounded_channel();
    let writing = tokio::spawn(write_out(sink, outgoing, session));
    let mut connection = Connection::new(live, agent, to_client);

    loop {
        tokio::select! {
            heard = stream.next() => match heard {
                Some(Ok(Frame::Binary(bytes))) => connection.hear(&bytes).await,
                Some(Ok(_)) => {}
                Some(Err(_)) | None => break,
            },
            scene = connection.access_changed() => connection.ask_again(scene).await,
        }
    }

    drop(connection);
    writing.abort();
}
