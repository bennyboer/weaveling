use std::error::Error;
use std::future;
use std::sync::Arc;

use async_trait::async_trait;
use authorizing::AccessWatch;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use eventsourcing::Agent;
use live_contract::{ACCESS_CHANGED, FrameDTO, NO_LONGER_ADMITTED, NoticeDTO};
use serde_json::json;

use crate::{Channel, Gate, Heard, Hearing};

pub struct Admission {
    pub(crate) watch: AccessWatch,
    pub(crate) gate: Arc<dyn Gate>,
    pub(crate) agent: Agent,
    pub(crate) about: String,
}

pub(crate) enum Access {
    Kept,
    Lost,
}

impl Admission {
    pub fn new(watch: AccessWatch, gate: Arc<dyn Gate>, agent: Agent, about: String) -> Self {
        Self {
            watch,
            gate,
            agent,
            about,
        }
    }

    async fn rechecked(&mut self) -> Access {
        self.watch.changed().await;
        match self.gate.admits(&self.agent, &self.about).await {
            Ok(true) => Access::Kept,
            _ => Access::Lost,
        }
    }
}

pub(crate) async fn rechecked(admission: &mut Option<Admission>) -> Access {
    match admission {
        Some(admission) => admission.rechecked().await,
        None => future::pending().await,
    }
}

fn access_changed() -> String {
    serde_json::to_string(&NoticeDTO {
        notice: ACCESS_CHANGED.to_owned(),
    })
    .expect("a notice is plain data")
}

#[async_trait]
pub trait Sieve: Send + Sync {
    async fn lets_through(&self, frame: &FrameDTO) -> Result<bool, Box<dyn Error + Send + Sync>>;
}

pub fn streaming<C: Channel>(upgrade: WebSocketUpgrade, hearing: Hearing<C>) -> Response {
    upgrade.on_upgrade(move |socket| forward(socket, Box::new(hearing), None, None))
}

pub(crate) fn streaming_while<C: Channel>(
    upgrade: WebSocketUpgrade,
    hearing: Hearing<C>,
    admission: Admission,
) -> Response {
    upgrade.on_upgrade(move |socket| forward(socket, Box::new(hearing), None, Some(admission)))
}

pub fn streaming_sifted<C: Channel>(
    upgrade: WebSocketUpgrade,
    hearing: Hearing<C>,
    sieve: Arc<dyn Sieve>,
) -> Response {
    upgrade.on_upgrade(move |socket| forward(socket, Box::new(hearing), Some(sieve), None))
}

pub fn streaming_with_version<C: Channel>(
    upgrade: WebSocketUpgrade,
    version: u64,
    hearing: Hearing<C>,
    admission: Admission,
) -> Response {
    let standing = json!({ "version": version }).to_string();

    upgrade.on_upgrade(move |mut socket| async move {
        if socket.send(Message::Text(standing.into())).await.is_ok() {
            forward(socket, Box::new(hearing), None, Some(admission)).await;
        }
    })
}

async fn forward(
    mut socket: WebSocket,
    mut hearing: Box<dyn Heard>,
    sieve: Option<Arc<dyn Sieve>>,
    mut admission: Option<Admission>,
) {
    loop {
        tokio::select! {
            access = rechecked(&mut admission) => match access {
                Access::Kept => {
                    if socket.send(Message::Text(access_changed().into())).await.is_err() {
                        return;
                    }
                }
                Access::Lost => {
                    _ = socket
                        .send(Message::Close(Some(CloseFrame {
                            code: NO_LONGER_ADMITTED,
                            reason: "no-longer-admitted".into(),
                        })))
                        .await;
                    return;
                }
            },
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
        State(broadcaster): State<Broadcaster<String>>,
        Path(about): Path<String>,
    ) -> Response {
        streaming(upgrade, broadcaster.hear(about.clone()))
    }

    async fn following_from_seven(
        upgrade: WebSocketUpgrade,
        State(broadcaster): State<Broadcaster<String>>,
        Path(about): Path<String>,
    ) -> Response {
        streaming_with_version(
            upgrade,
            7,
            broadcaster.hear(about.clone()),
            Admission::new(AccessWatch::still(), Arc::new(Open), Agent::Local, about),
        )
    }

    struct Open;

    #[async_trait]
    impl Gate for Open {
        async fn admits(
            &self,
            _agent: &Agent,
            _about: &str,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            Ok(true)
        }
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
        State(broadcaster): State<Broadcaster<String>>,
        Path(about): Path<String>,
    ) -> Response {
        streaming_sifted(upgrade, broadcaster.hear(about.clone()), Arc::new(Mine))
    }

    fn served(broadcaster: Broadcaster<String>) -> TestServer {
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

        broadcaster.tell("projects".to_owned(), of("theirs"));
        broadcaster.tell("projects".to_owned(), of("mine, the first"));
        broadcaster.tell("projects".to_owned(), of("theirs again"));
        broadcaster.tell("projects".to_owned(), of("mine, the second"));

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

        broadcaster.tell("projects".to_owned(), "mine, but no frame".to_owned());
        broadcaster.tell("projects".to_owned(), of("mine, after"));

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

        broadcaster.tell("projects".to_owned(), of("broken"));
        broadcaster.tell("projects".to_owned(), of("mine, after"));

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

        broadcaster.tell("outline_2".to_owned(), "theirs".to_owned());
        broadcaster.tell("outline_1".to_owned(), "mine".to_owned());

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

        broadcaster.tell("outline_1".to_owned(), "mine".to_owned());

        assert_eq!(socket.receive_text().await, r#"{"version":7}"#);
        assert_eq!(socket.receive_text().await, "mine");
    }
}
