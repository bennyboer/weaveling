use std::time::Duration;

use axum_test::{TestServer, TestWebSocket, WsMessage};
use live_contract::{AskedDTO, ToldDTO};
use serde_json::Value;

const SETTLING: Duration = Duration::from_millis(50);
const FRESH: Duration = Duration::from_secs(1);
const QUIET: Duration = Duration::from_millis(150);

/// A test client of the shared socket.
/// It subscribes like the browser does and reads what it receives.
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

    /// Subscribes under `id` to `topic` about `about`.
    /// Then it gives the server a moment to process the subscription.
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

    /// The next message received, skipping confirmations that a subscription is followed.
    /// Fails unless it arrives within a second.
    pub async fn told(&mut self) -> ToldDTO {
        loop {
            match self.anything_told().await {
                ToldDTO::Following { .. } => {}
                told => return told,
            }
        }
    }

    /// Waits until the subscription under `id` is followed.
    /// Fails if anything else arrives first.
    pub async fn following(&mut self, id: &str) {
        match self.anything_told().await {
            ToldDTO::Following { id: followed } if followed == id => {}
            other => panic!("{id} followed, not {other:?}"),
        }
    }

    async fn anything_told(&mut self) -> ToldDTO {
        tokio::time::timeout(FRESH, self.socket.receive_json())
            .await
            .expect("told while it is fresh")
    }

    /// The next frame received, as JSON to index into.
    /// Fails on anything else.
    pub async fn frame(&mut self) -> Value {
        match self.told().await {
            ToldDTO::Frame { frame, .. } => {
                serde_json::to_value(frame).expect("a frame is plain data")
            }
            other => panic!("a frame, not {other:?}"),
        }
    }

    /// Why the next message ended a subscription.
    /// Fails on anything else.
    pub async fn ended(&mut self) -> String {
        match self.told().await {
            ToldDTO::Ended { why, .. } => why,
            other => panic!("an end, not {other:?}"),
        }
    }

    /// Whether nothing arrives for a short while, apart from confirmations that a subscription is followed.
    /// Anything received meanwhile is read and dropped.
    pub async fn silent(&mut self) -> bool {
        loop {
            match tokio::time::timeout(QUIET, self.socket.receive_json::<ToldDTO>()).await {
                Err(_) => return true,
                Ok(ToldDTO::Following { .. }) => {}
                Ok(_) => return false,
            }
        }
    }

    pub async fn nothing(&mut self) {
        assert!(self.silent().await, "nothing is told");
    }

    /// The code the server closed the socket with, if it closes within a second.
    pub async fn closed_as(&mut self) -> Option<u16> {
        loop {
            match tokio::time::timeout(FRESH, self.socket.receive_message()).await {
                Ok(WsMessage::Close(Some(frame))) => return Some(u16::from(frame.code)),
                Ok(WsMessage::Text(text))
                    if matches!(
                        serde_json::from_str::<ToldDTO>(&text),
                        Ok(ToldDTO::Following { .. })
                    ) => {}
                _ => return None,
            }
        }
    }

    /// Sends `text` as it is, whatever it says.
    pub async fn send_text(&mut self, text: String) {
        self.socket.send_text(text).await;
    }

    pub async fn close(self) {
        self.socket.close().await;
        tokio::time::sleep(SETTLING).await;
    }
}
