use async_trait::async_trait;
use eventpublishing::{PublishedEvent, published_in};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use serde_json::{Value, json};

use crate::Broadcaster;

pub struct BroadcastChangesListener {
    named: ListenerName,
    listens_to: Subscription,
    about: Option<String>,
    broadcaster: Broadcaster,
}

impl BroadcastChangesListener {
    pub fn new(named: &str, listens_to: &str, broadcaster: Broadcaster) -> Self {
        Self {
            named: ListenerName::parse(named)
                .expect("a broadcast's name is written at compile time"),
            listens_to: Subscription::parse(listens_to)
                .expect("a broadcast's subscription is written at compile time"),
            about: None,
            broadcaster,
        }
    }

    pub fn under(named: &str, listens_to: &str, about: &str, broadcaster: Broadcaster) -> Self {
        Self {
            about: Some(about.to_owned()),
            ..Self::new(named, listens_to, broadcaster)
        }
    }
}

pub fn frame_of(published: &PublishedEvent<Value>) -> String {
    json!({
        "of": published.aggregate.id,
        "version": published.aggregate.version,
        "at": published.occurred_at,
        "event": published.event.body,
    })
    .to_string()
}

#[async_trait]
impl Listener for BroadcastChangesListener {
    fn named(&self) -> ListenerName {
        self.named.clone()
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![self.listens_to.clone()]
    }

    fn when_refused(&self) -> &'static str {
        "A change was not passed on to whoever has it open."
    }

    fn delivery(&self) -> Delivery {
        Delivery::Fleeting
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        if let Ok(published) = published_in::<Value>(message) {
            let frame = frame_of(&published);
            if let Some(about) = &self.about {
                self.broadcaster.tell(about, frame.clone());
            }
            self.broadcaster.tell(&published.aggregate.id, frame);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use messaging::RoutingKey;
    use time::OffsetDateTime;

    use super::*;

    fn retitled(outline: &str, version: u64) -> Message {
        Message::opening(
            RoutingKey::parse("outline.section.retitled").expect("a plain key"),
            json!({
                "event": { "version": 0, "name": "SECTION_RETITLED", "section": "section_1", "title": "Part One" },
                "aggregate": { "id": outline, "kind": "outline", "version": version },
                "agent": { "kind": "local", "id": null },
                "occurred_at": "1970-01-01T00:16:40Z",
            }),
            OffsetDateTime::UNIX_EPOCH,
        )
    }

    fn broadcasting(broadcaster: &Broadcaster) -> BroadcastChangesListener {
        BroadcastChangesListener::new(
            "broadcast-outline-changes",
            "outline.#",
            broadcaster.clone(),
        )
    }

    #[tokio::test]
    async fn a_published_change_is_passed_on_as_its_version_and_its_event() {
        let broadcaster = Broadcaster::new();
        let mut hearing = broadcaster.hear("outline_1");

        broadcasting(&broadcaster)
            .handle(&retitled("outline_1", 7))
            .await
            .expect("passing on is not a failure");

        let told = tokio::time::timeout(Duration::from_millis(100), hearing.next())
            .await
            .expect("the change is passed on at once")
            .expect("the broadcaster still tells");
        let frame: Value = serde_json::from_str(&told).expect("a frame is JSON");
        assert_eq!(
            frame,
            json!({
                "of": "outline_1",
                "version": 7,
                "at": "1970-01-01T00:16:40Z",
                "event": { "name": "SECTION_RETITLED", "section": "section_1", "title": "Part One" },
            })
        );
    }

    #[tokio::test]
    async fn a_message_that_is_no_published_change_is_let_go() {
        let broadcaster = Broadcaster::new();
        let mut hearing = broadcaster.hear("outline_1");
        let stray = Message::opening(
            RoutingKey::parse("outline.started").expect("a plain key"),
            json!({ "nothing": "useful" }),
            OffsetDateTime::UNIX_EPOCH,
        );

        broadcasting(&broadcaster)
            .handle(&stray)
            .await
            .expect("a stray message is no reason to stop");

        assert!(
            tokio::time::timeout(Duration::from_millis(100), hearing.next())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn a_broadcast_under_one_name_tells_every_aggregate_s_change_there_and_under_its_own() {
        let broadcaster = Broadcaster::new();
        let mut everyone = broadcaster.hear("outlines");
        let mut one = broadcaster.hear("outline_1");

        BroadcastChangesListener::under(
            "broadcast-outline-changes",
            "outline.#",
            "outlines",
            broadcaster.clone(),
        )
        .handle(&retitled("outline_1", 7))
        .await
        .expect("passing on is not a failure");

        let told = tokio::time::timeout(Duration::from_millis(100), everyone.next())
            .await
            .expect("the change is passed on at once")
            .expect("the broadcaster still tells");
        let frame: Value = serde_json::from_str(&told).expect("a frame is JSON");
        assert_eq!(frame["of"], "outline_1");
        let its_own = tokio::time::timeout(Duration::from_millis(100), one.next())
            .await
            .expect("whoever follows the one aggregate is told too")
            .expect("the broadcaster still tells");
        assert_eq!(its_own, told);
    }

    #[test]
    fn broadcasting_is_fleeting_and_hears_what_it_was_given() {
        let broadcast = broadcasting(&Broadcaster::new());

        assert_eq!(
            broadcast.delivery(),
            Delivery::Fleeting,
            "a change nobody had open is not worth keeping for later"
        );
        assert!(broadcast.hears(&RoutingKey::parse("outline.section.moved").expect("a plain key")));
        assert!(!broadcast.hears(&RoutingKey::parse("board.piece.moved").expect("a plain key")));
        assert_eq!(broadcast.named().to_string(), "broadcast-outline-changes");
    }
}
