use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use serde_json::json;

use crate::Hearing;

pub fn streaming(upgrade: WebSocketUpgrade, hearing: Hearing) -> Response {
    upgrade.on_upgrade(move |socket| forward(socket, hearing))
}

pub fn streaming_from(upgrade: WebSocketUpgrade, version: u64, hearing: Hearing) -> Response {
    let standing = json!({ "version": version }).to_string();

    upgrade.on_upgrade(move |mut socket| async move {
        if socket.send(Message::Text(standing.into())).await.is_ok() {
            forward(socket, hearing).await;
        }
    })
}

async fn forward(mut socket: WebSocket, mut hearing: Hearing) {
    loop {
        tokio::select! {
            told = hearing.next() => {
                let Some(frame) = told else {
                    return;
                };
                if socket.send(Message::Text(frame.as_ref().into())).await.is_err() {
                    return;
                }
            }
            heard = socket.recv() => match heard {
                Some(Ok(Message::Close(_)) | Err(_)) | None => return,
                Some(Ok(_)) => {}
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::extract::{Path, State};
    use axum::routing;
    use axum_test::TestServer;

    use super::*;
    use crate::Broadcaster;

    async fn following(
        upgrade: WebSocketUpgrade,
        State(broadcaster): State<Broadcaster>,
        Path(about): Path<String>,
    ) -> Response {
        streaming(upgrade, broadcaster.hear(&about))
    }

    async fn following_from_seven(
        upgrade: WebSocketUpgrade,
        State(broadcaster): State<Broadcaster>,
        Path(about): Path<String>,
    ) -> Response {
        streaming_from(upgrade, 7, broadcaster.hear(&about))
    }

    fn served(broadcaster: Broadcaster) -> TestServer {
        TestServer::builder().http_transport().build(
            Router::new()
                .route("/live/{about}", routing::get(following))
                .route("/standing/{about}", routing::get(following_from_seven))
                .with_state(broadcaster),
        )
    }

    #[tokio::test]
    async fn a_socket_passes_on_what_is_told_about_what_it_follows() {
        let broadcaster = Broadcaster::new();
        let server = served(broadcaster.clone());
        let mut socket = server
            .get_websocket("/live/outline_1")
            .await
            .into_websocket()
            .await;

        broadcaster.tell("outline_2", "theirs".to_owned());
        broadcaster.tell("outline_1", "mine".to_owned());

        assert_eq!(socket.receive_text().await, "mine");
    }

    #[tokio::test]
    async fn a_socket_from_a_version_says_that_version_before_anything_told() {
        let broadcaster = Broadcaster::new();
        let server = served(broadcaster.clone());
        let mut socket = server
            .get_websocket("/standing/outline_1")
            .await
            .into_websocket()
            .await;

        broadcaster.tell("outline_1", "mine".to_owned());

        assert_eq!(socket.receive_text().await, r#"{"version":7}"#);
        assert_eq!(socket.receive_text().await, "mine");
    }
}
