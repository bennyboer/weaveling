use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use live_contract::FrameDTO;
use serde_json::json;

use crate::Hearing;

#[async_trait]
pub trait Sieve: Send + Sync {
    async fn lets_through(&self, frame: &FrameDTO) -> Result<bool, Box<dyn Error + Send + Sync>>;
}

pub fn streaming(upgrade: WebSocketUpgrade, hearing: Hearing) -> Response {
    upgrade.on_upgrade(move |socket| forward(socket, hearing, None))
}

pub fn streaming_sifted(
    upgrade: WebSocketUpgrade,
    hearing: Hearing,
    sieve: Arc<dyn Sieve>,
) -> Response {
    upgrade.on_upgrade(move |socket| forward(socket, hearing, Some(sieve)))
}

pub fn streaming_from(upgrade: WebSocketUpgrade, version: u64, hearing: Hearing) -> Response {
    let standing = json!({ "version": version }).to_string();

    upgrade.on_upgrade(move |mut socket| async move {
        if socket.send(Message::Text(standing.into())).await.is_ok() {
            forward(socket, hearing, None).await;
        }
    })
}

async fn forward(mut socket: WebSocket, mut hearing: Hearing, sieve: Option<Arc<dyn Sieve>>) {
    loop {
        tokio::select! {
            told = hearing.next() => {
                let Some(frame) = told else {
                    return;
                };
                if let Some(sieve) = &sieve {
                    let Ok(read) = serde_json::from_str::<FrameDTO>(&frame) else {
                        return;
                    };
                    match sieve.lets_through(&read).await {
                        Ok(true) => {}
                        Ok(false) => continue,
                        Err(_) => return,
                    }
                }
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

    struct Mine;

    #[async_trait]
    impl Sieve for Mine {
        async fn lets_through(
            &self,
            frame: &FrameDTO,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            match frame.of.as_deref() {
                Some("broken") => Err("the sieve cannot tell".into()),
                of => Ok(of.is_some_and(|of| of.starts_with("mine"))),
            }
        }
    }

    fn of(about: &str) -> String {
        json!({ "of": about, "version": 1, "event": {} }).to_string()
    }

    fn of_in(heard: &str) -> String {
        serde_json::from_str::<FrameDTO>(heard)
            .expect("a frame")
            .of
            .expect("a frame says what it is of")
    }

    async fn following_mine(
        upgrade: WebSocketUpgrade,
        State(broadcaster): State<Broadcaster>,
        Path(about): Path<String>,
    ) -> Response {
        streaming_sifted(upgrade, broadcaster.hear(&about), Arc::new(Mine))
    }

    fn served(broadcaster: Broadcaster) -> TestServer {
        TestServer::builder().http_transport().build(
            Router::new()
                .route("/live/{about}", routing::get(following))
                .route("/standing/{about}", routing::get(following_from_seven))
                .route("/mine/{about}", routing::get(following_mine))
                .with_state(broadcaster),
        )
    }

    #[tokio::test]
    async fn a_sifted_socket_passes_on_only_what_its_sieve_lets_through() {
        let broadcaster = Broadcaster::new();
        let server = served(broadcaster.clone());
        let mut socket = server
            .get_websocket("/mine/projects")
            .await
            .into_websocket()
            .await;

        broadcaster.tell("projects", of("theirs"));
        broadcaster.tell("projects", of("mine, the first"));
        broadcaster.tell("projects", of("theirs again"));
        broadcaster.tell("projects", of("mine, the second"));

        assert_eq!(of_in(&socket.receive_text().await), "mine, the first");
        assert_eq!(of_in(&socket.receive_text().await), "mine, the second");
    }

    #[tokio::test]
    async fn a_frame_that_is_no_frame_ends_a_sifted_socket() {
        let broadcaster = Broadcaster::new();
        let server = served(broadcaster.clone());
        let mut socket = server
            .get_websocket("/mine/projects")
            .await
            .into_websocket()
            .await;

        broadcaster.tell("projects", "mine, but no frame".to_owned());
        broadcaster.tell("projects", of("mine, after"));

        assert!(
            tokio::spawn(async move { socket.receive_text().await })
                .await
                .is_err(),
            "what cannot be read cannot be judged"
        );
    }

    #[tokio::test]
    async fn a_sieve_that_cannot_tell_ends_the_socket_so_its_client_reloads() {
        let broadcaster = Broadcaster::new();
        let server = served(broadcaster.clone());
        let mut socket = server
            .get_websocket("/mine/projects")
            .await
            .into_websocket()
            .await;

        broadcaster.tell("projects", of("broken"));
        broadcaster.tell("projects", of("mine, after"));

        assert!(
            tokio::spawn(async move { socket.receive_text().await })
                .await
                .is_err(),
            "a frame it cannot judge may be one the follower needed, so it is not skipped quietly"
        );
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
