use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;

use crate::listening::{Publisher, Undelivered};
use crate::message::Message;

#[derive(Default)]
pub struct FakePublisher {
    heard: Mutex<Vec<Message>>,
    refusing: AtomicBool,
}

impl FakePublisher {
    pub fn refusing(&self, refusing: bool) {
        self.refusing.store(refusing, Ordering::SeqCst);
    }

    pub fn heard(&self) -> Vec<Message> {
        self.heard
            .lock()
            .expect("what was heard is never poisoned")
            .clone()
    }
}

#[async_trait]
impl Publisher for FakePublisher {
    async fn publish(&self, message: Message) -> Result<(), Undelivered> {
        if self.refusing.load(Ordering::SeqCst) {
            return Err(Undelivered::because(message.routing, Refused));
        }
        self.heard
            .lock()
            .expect("what was heard is never poisoned")
            .push(message);

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("the fake publisher was told to refuse")]
struct Refused;

#[cfg(test)]
mod tests {
    use serde_json::json;
    use time::OffsetDateTime;

    use super::*;
    use crate::routing::RoutingKey;

    fn a_message() -> Message {
        Message::opening(
            RoutingKey::parse("idea.captured").expect("a plain key"),
            json!({}),
            OffsetDateTime::UNIX_EPOCH,
        )
    }

    #[tokio::test]
    async fn it_hears_what_is_published_until_told_to_refuse() {
        let publisher = FakePublisher::default();
        let first = a_message();

        publisher
            .publish(first.clone())
            .await
            .expect("it listens by default");
        publisher.refusing(true);
        let refused = publisher.publish(a_message()).await;

        assert!(refused.is_err());
        assert_eq!(publisher.heard(), [first]);
    }
}
