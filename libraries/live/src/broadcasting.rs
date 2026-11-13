use async_trait::async_trait;
use eventpublishing::PublishedEvent;
use live_contract::FrameDTO;
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use serde_json::Value;

use crate::{Broadcaster, Channel};

/// Passes each published change it receives to the broadcaster, as a frame.
/// It sends the frame on every channel that `channels` names for the change.
pub struct BroadcastChangesListener<C> {
    named: ListenerName,
    listens_to: Subscription,
    channels: fn(&PublishedEvent<Value>) -> Vec<C>,
    broadcaster: Broadcaster<C>,
}

impl<C: Channel> BroadcastChangesListener<C> {
    pub fn new(
        named: &str,
        listens_to: &str,
        broadcaster: Broadcaster<C>,
        channels: fn(&PublishedEvent<Value>) -> Vec<C>,
    ) -> Self {
        Self {
            named: ListenerName::parse(named)
                .expect("a broadcast's name is written at compile time"),
            listens_to: Subscription::parse(listens_to)
                .expect("a broadcast's subscription is written at compile time"),
            channels,
            broadcaster,
        }
    }
}

/// The frame a published change is sent as.
/// It holds what the change is about, its version, when it happened and its event.
pub fn frame_of(published: &PublishedEvent<Value>) -> String {
    serde_json::to_string(&FrameDTO {
        of: Some(published.aggregate.id.clone()),
        version: published.aggregate.version,
        at: Some(published.occurred_at.clone()),
        event: published.event.body.clone(),
    })
    .expect("a frame is plain data and cannot fail to serialize")
}

#[async_trait]
impl<C: Channel> Listener for BroadcastChangesListener<C> {
    fn named(&self) -> ListenerName {
        self.named.clone()
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![self.listens_to.clone()]
    }

    fn when_refused(&self) -> &'static str {
        "A change was not sent to the people who have it open."
    }

    fn delivery(&self) -> Delivery {
        Delivery::Fleeting
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        if let Ok(published) = eventpublishing::published_in::<Value>(message) {
            let frame = frame_of(&published);
            for channel in (self.channels)(&published) {
                self.broadcaster.tell(channel, frame.clone());
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use messaging::RoutingKey;
    use serde_json::json;
    use time::OffsetDateTime;

    use super::*;
    use crate::Frames;

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

    fn broadcasting(broadcaster: &Broadcaster<String>) -> BroadcastChangesListener<String> {
        BroadcastChangesListener::new(
            "broadcast-outline-changes",
            "outline.#",
            broadcaster.clone(),
            |published| vec![published.aggregate.id.clone()],
        )
    }

    #[tokio::test]
    async fn a_published_change_is_passed_on_as_its_version_and_its_event() {
        // given: someone hearing an outline
        let broadcaster = Broadcaster::new();
        let mut hearing = broadcaster.hear("outline_1".to_owned());

        // when: a change to it is published
        broadcasting(&broadcaster)
            .handle(&retitled("outline_1", 7))
            .await
            .expect("passing on is not a failure");

        // then: it hears the change's version and event
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
        // given: someone hearing an outline
        let broadcaster = Broadcaster::new();
        let mut hearing = broadcaster.hear("outline_1".to_owned());
        // and: a message that is no published change
        let stray = Message::opening(
            RoutingKey::parse("outline.started").expect("a plain key"),
            json!({ "nothing": "useful" }),
            OffsetDateTime::UNIX_EPOCH,
        );

        // when: it is handled
        broadcasting(&broadcaster)
            .handle(&stray)
            .await
            .expect("a stray message is no reason to stop");

        // then: nothing is heard
        assert!(
            tokio::time::timeout(Duration::from_millis(100), hearing.next())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn a_change_is_told_on_every_channel_it_is_read_to_belong_to() {
        // given: someone hearing all outlines
        let broadcaster = Broadcaster::new();
        let mut everyone = broadcaster.hear("outlines".to_owned());
        // and: someone hearing one of them
        let mut one = broadcaster.hear("outline_1".to_owned());

        // when: a change to that one is published to both channels
        BroadcastChangesListener::new(
            "broadcast-outline-changes",
            "outline.#",
            broadcaster.clone(),
            |published| vec!["outlines".to_owned(), published.aggregate.id.clone()],
        )
        .handle(&retitled("outline_1", 7))
        .await
        .expect("passing on is not a failure");

        // then: whoever hears all outlines hears it
        let told = tokio::time::timeout(Duration::from_millis(100), everyone.next())
            .await
            .expect("the change is passed on at once")
            .expect("the broadcaster still tells");
        let frame: Value = serde_json::from_str(&told).expect("a frame is JSON");
        assert_eq!(frame["of"], "outline_1");
        // and: whoever hears the one hears the same
        let its_own = tokio::time::timeout(Duration::from_millis(100), one.next())
            .await
            .expect("whoever follows the one aggregate is told too")
            .expect("the broadcaster still tells");
        assert_eq!(its_own, told);
    }

    #[test]
    fn broadcasting_is_fleeting_and_hears_what_it_was_given() {
        // given: a broadcasting listener
        let broadcast = broadcasting(&Broadcaster::<String>::new());

        // then: it is fleeting, hears its keys and is named as given
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
