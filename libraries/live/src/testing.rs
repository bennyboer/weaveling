use std::time::Duration;

use axum_test::{TestServer, TestWebSocket, WsMessage};
use live_contract::{AskedDTO, ToldDTO};
use serde_json::Value;

const SETTLING: Duration = Duration::from_millis(50);
const FRESH: Duration = Duration::from_secs(1);
const QUIET: Duration = Duration::from_millis(150);

/// A client of the one socket in tests: it subscribes as the browser does and reads what it is told.
pub struct Subscriber {
    socket: TestWebSocket,
}

impl Subscriber {
    /// Opens `/live` on `server`.
    pub async fn on(server: &TestServer) -> Self {
        Self {
            socket: server.get_websocket("/live").await.into_websocket().await,
        }
    }

    /// Subscribes under `id` to `topic` about `about`, and gives the server a moment to take it in.
    pub async fn subscribe(&mut self, id: &str, topic: &str, about: &[(&str, &str)]) {
        self.socket
            .send_json(&AskedDTO::Subscribe {
                id: id.to_owned(),
                topic: topic.to_owned(),
                about: about
                    .iter()
                    .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                    .collect(),
            })
            .await;
        tokio::time::sleep(SETTLING).await;
    }

    pub async fn unsubscribe(&mut self, id: &str) {
        self.socket
            .send_json(&AskedDTO::Unsubscribe { id: id.to_owned() })
            .await;
        tokio::time::sleep(SETTLING).await;
    }

    /// The next thing told, whatever it is, failing unless it comes within a second.
    pub async fn told(&mut self) -> ToldDTO {
        tokio::time::timeout(FRESH, self.socket.receive_json())
            .await
            .expect("told while it is fresh")
    }

    /// The next frame told, as JSON to index into, failing on anything else.
    pub async fn frame(&mut self) -> Value {
        match self.told().await {
            ToldDTO::Frame { frame, .. } => {
                serde_json::to_value(frame).expect("a frame is plain data")
            }
            other => panic!("a frame, not {other:?}"),
        }
    }

    /// Why the next thing told ended a subscription, failing on anything else.
    pub async fn ended(&mut self) -> String {
        match self.told().await {
            ToldDTO::Ended { why, .. } => why,
            other => panic!("an end, not {other:?}"),
        }
    }

    /// Whether nothing is told for a short while; whatever is told meanwhile is read and gone.
    pub async fn silent(&mut self) -> bool {
        tokio::time::timeout(QUIET, self.socket.receive_text())
            .await
            .is_err()
    }

    pub async fn nothing(&mut self) {
        assert!(self.silent().await, "nothing is told");
    }

    /// The code the server closed the socket with, if it closes within a second.
    pub async fn closed_as(&mut self) -> Option<u16> {
        match tokio::time::timeout(FRESH, self.socket.receive_message()).await {
            Ok(WsMessage::Close(Some(frame))) => Some(u16::from(frame.code)),
            _ => None,
        }
    }

    pub async fn close(self) {
        self.socket.close().await;
        tokio::time::sleep(SETTLING).await;
    }
}
