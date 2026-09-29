use std::sync::{Arc, RwLock};

use async_trait::async_trait;

use crate::delivering::Deliveries;
use crate::listening::{Listener, ListenerName, Publisher, Undelivered};
use crate::message::Message;

pub struct InProcessDispatcher {
    listeners: RwLock<Vec<Arc<dyn Listener>>>,
    deliveries: Arc<dyn Deliveries>,
}

impl InProcessDispatcher {
    pub fn queueing_to(deliveries: Arc<dyn Deliveries>) -> Self {
        Self {
            listeners: RwLock::new(Vec::new()),
            deliveries,
        }
    }

    pub fn listen(&self, listener: Arc<dyn Listener>) {
        let mut listeners = self.listeners.write().expect("messaging lock poisoned");

        assert!(
            !listeners
                .iter()
                .any(|held| held.named() == listener.named()),
            "{} is already listening: a name is a queue, so a second one under it would              quietly eat the first one's messages",
            listener.named()
        );

        listeners.push(listener);
    }

    pub(crate) fn named(&self, wanted: &ListenerName) -> Option<Arc<dyn Listener>> {
        self.listeners
            .read()
            .expect("messaging lock poisoned")
            .iter()
            .find(|listener| &listener.named() == wanted)
            .cloned()
    }

    fn interested_in(&self, message: &Message) -> Vec<Arc<dyn Listener>> {
        self.listeners
            .read()
            .expect("messaging lock poisoned")
            .iter()
            .filter(|listener| listener.hears(&message.routing))
            .cloned()
            .collect()
    }
}

#[async_trait]
impl Publisher for InProcessDispatcher {
    async fn publish(&self, message: Message) -> Result<(), Undelivered> {
        for listener in self.interested_in(&message) {
            self.deliveries
                .enqueue(&listener.named(), &message)
                .await
                .map_err(|why| Undelivered::because(message.routing.clone(), why))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;
    use thiserror::Error;
    use time::{Duration, OffsetDateTime};

    use clock::FixedClock;

    use super::*;
    use crate::consuming::DeliveryConsumer;
    use crate::deliveries::InMemoryDeliveries;
    use crate::listening::{Delivery, NotHandled};
    use crate::routing::{RoutingKey, Subscription};

    #[derive(Debug, Error)]
    #[error("this listener always refuses")]
    struct Refused;

    struct Overheard {
        name: ListenerName,
        subscriptions: Vec<Subscription>,
        delivery: Delivery,
        heard: Mutex<Vec<RoutingKey>>,
        refuses: bool,
    }

    #[async_trait]
    impl Listener for Overheard {
        fn named(&self) -> ListenerName {
            self.name.clone()
        }

        fn listens_to(&self) -> Vec<Subscription> {
            self.subscriptions.clone()
        }

        fn delivery(&self) -> Delivery {
            self.delivery
        }

        async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
            self.heard
                .lock()
                .expect("heard lock poisoned")
                .push(message.routing.clone());

            if self.refuses {
                return Err(NotHandled::because(
                    self.named(),
                    message.routing.clone(),
                    Refused,
                ));
            }

            Ok(())
        }
    }

    impl Overheard {
        fn named(name: &str, to: &str, delivery: Delivery, refuses: bool) -> Arc<Self> {
            Arc::new(Self {
                name: ListenerName::parse(name).expect("a plain name is fine"),
                subscriptions: vec![Subscription::parse(to).expect("a plain pattern is fine")],
                delivery,
                heard: Mutex::new(Vec::new()),
                refuses,
            })
        }

        fn listening_to_both(first: &str, second: &str) -> Arc<Self> {
            Arc::new(Self {
                name: ListenerName::parse("both").expect("a plain name is fine"),
                subscriptions: vec![
                    Subscription::parse(first).expect("a plain pattern is fine"),
                    Subscription::parse(second).expect("a plain pattern is fine"),
                ],
                delivery: Delivery::Kept,
                heard: Mutex::new(Vec::new()),
                refuses: false,
            })
        }

        fn listening(to: &str) -> Arc<Self> {
            Self::named("listening", to, Delivery::Kept, false)
        }

        fn what_it_heard(&self) -> Vec<String> {
            self.heard
                .lock()
                .expect("heard lock poisoned")
                .iter()
                .map(ToString::to_string)
                .collect()
        }
    }

    struct Wired {
        dispatcher: Arc<InProcessDispatcher>,
        deliveries: Arc<InMemoryDeliveries>,
        clock: Arc<FixedClock>,
    }

    impl Wired {
        fn listen(&self, listener: Arc<dyn Listener>) {
            self.dispatcher.listen(listener);
        }

        async fn publish(&self, message: Message) -> Result<(), Undelivered> {
            let handed = self.dispatcher.publish(message).await;
            self.drain().await;

            handed
        }

        async fn drain(&self) {
            DeliveryConsumer::new(
                self.dispatcher.clone(),
                self.deliveries.clone(),
                self.clock.clone(),
            )
            .drain(64)
            .await;
        }

        async fn dead(&self) -> Vec<(String, String)> {
            self.deliveries
                .dead_letters()
                .await
                .expect("reading the dead letters should succeed")
                .into_iter()
                .map(|dead| (dead.listener.to_string(), dead.message.routing.to_string()))
                .collect()
        }
    }

    fn a_workbench() -> Wired {
        let deliveries = Arc::new(InMemoryDeliveries::new());
        let clock = Arc::new(FixedClock::new(at(2_000)));

        Wired {
            dispatcher: Arc::new(InProcessDispatcher::queueing_to(deliveries.clone())),
            deliveries,
            clock,
        }
    }

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn saying(routing: &str) -> Message {
        Message::opening(
            RoutingKey::parse(routing).expect("a plain key is fine"),
            json!({}),
            at(1_000),
        )
    }

    #[tokio::test]
    async fn a_listener_hears_what_it_subscribed_to() {
        let dispatcher = a_workbench();
        let listener = Overheard::listening("piece.captured");
        dispatcher.listen(listener.clone());

        dispatcher
            .publish(saying("piece.captured"))
            .await
            .expect("publishing should succeed");

        assert_eq!(listener.what_it_heard(), vec!["piece.captured"]);
    }

    #[tokio::test]
    async fn a_listener_hears_nothing_it_did_not_subscribe_to() {
        let dispatcher = a_workbench();
        let listener = Overheard::listening("piece.captured");
        dispatcher.listen(listener.clone());

        dispatcher
            .publish(saying("board.pinned"))
            .await
            .expect("publishing should succeed");

        assert!(listener.what_it_heard().is_empty());
    }

    #[tokio::test]
    async fn everyone_interested_hears_the_same_message() {
        let dispatcher = a_workbench();
        let exact = Overheard::named("exact", "piece.captured", Delivery::Kept, false);
        let wildcard = Overheard::named("wildcard", "piece.*", Delivery::Kept, false);
        let everything = Overheard::named("everything", "#", Delivery::Kept, false);
        dispatcher.listen(exact.clone());
        dispatcher.listen(wildcard.clone());
        dispatcher.listen(everything.clone());

        dispatcher
            .publish(saying("piece.captured"))
            .await
            .expect("publishing should succeed");

        assert_eq!(exact.what_it_heard().len(), 1);
        assert_eq!(wildcard.what_it_heard().len(), 1);
        assert_eq!(everything.what_it_heard().len(), 1);
    }

    #[tokio::test]
    #[should_panic(expected = "is already listening")]
    async fn two_listeners_may_not_share_a_name() {
        let dispatcher = a_workbench();
        dispatcher.listen(Overheard::listening("piece.captured"));

        dispatcher.listen(Overheard::listening("piece.retitled"));
    }

    #[tokio::test]
    async fn a_listener_may_bind_more_than_one_key() {
        let dispatcher = a_workbench();
        let listener = Overheard::listening_to_both("board.piece.pinned", "board.piece.unpinned");
        dispatcher.listen(listener.clone());

        for routing in [
            "board.piece.pinned",
            "board.piece.unpinned",
            "board.piece.moved",
        ] {
            dispatcher
                .publish(saying(routing))
                .await
                .expect("publishing should succeed");
        }

        assert_eq!(
            listener.what_it_heard(),
            vec!["board.piece.pinned", "board.piece.unpinned"],
            "several bindings on one queue is what a broker does, and the key between them is left out"
        );
    }

    #[tokio::test]
    async fn a_message_matching_two_bindings_arrives_once() {
        let dispatcher = a_workbench();
        let listener = Overheard::listening_to_both("board.#", "board.piece.pinned");
        dispatcher.listen(listener.clone());

        dispatcher
            .publish(saying("board.piece.pinned"))
            .await
            .expect("publishing should succeed");

        assert_eq!(
            listener.what_it_heard().len(),
            1,
            "a queue bound twice still receives a message once, and so must a listener"
        );
    }

    #[tokio::test]
    async fn a_message_nobody_wants_is_not_a_failure() {
        let dispatcher = a_workbench();

        dispatcher
            .publish(saying("piece.captured"))
            .await
            .expect("nobody listening is not an error, it is just quiet");
    }

    #[tokio::test]
    async fn a_listener_is_kept_unless_it_says_otherwise() {
        struct Quiet;

        #[async_trait]
        impl Listener for Quiet {
            fn named(&self) -> ListenerName {
                ListenerName::parse("quiet").expect("a plain name is fine")
            }

            fn listens_to(&self) -> Vec<Subscription> {
                vec![Subscription::parse("#").expect("a plain pattern is fine")]
            }

            async fn handle(&self, _message: &Message) -> Result<(), NotHandled> {
                Ok(())
            }
        }

        assert_eq!(
            Quiet.delivery(),
            Delivery::Kept,
            "losing a message must be chosen, never inherited"
        );
    }

    #[tokio::test]
    async fn nothing_is_dead_lettered_when_every_listener_copes() {
        let dispatcher = a_workbench();
        dispatcher.listen(Overheard::listening("piece.captured"));

        dispatcher
            .publish(saying("piece.captured"))
            .await
            .expect("publishing should succeed");

        assert!(dispatcher.dead().await.is_empty());
        assert_eq!(
            dispatcher
                .deliveries
                .waiting()
                .await
                .expect("counting should succeed"),
            0,
            "a delivery that was taken is done with, not left for the next drain"
        );
    }

    #[tokio::test]
    async fn a_listener_may_publish_while_being_told() {
        struct Echoing {
            dispatcher: Arc<InProcessDispatcher>,
        }

        #[async_trait]
        impl Listener for Echoing {
            fn named(&self) -> ListenerName {
                ListenerName::parse("echoing").expect("a plain name is fine")
            }

            fn listens_to(&self) -> Vec<Subscription> {
                vec![Subscription::parse("piece.captured").expect("a plain pattern is fine")]
            }

            async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
                let answer = message.answering(
                    RoutingKey::parse("board.pinned").expect("a plain key is fine"),
                    json!({}),
                    at(1_001),
                );
                let _ = self.dispatcher.publish(answer).await;

                Ok(())
            }
        }

        let dispatcher = a_workbench();
        let onward = Overheard::listening("board.pinned");
        dispatcher.listen(onward.clone());
        dispatcher.listen(Arc::new(Echoing {
            dispatcher: dispatcher.dispatcher.clone(),
        }));

        dispatcher
            .publish(saying("piece.captured"))
            .await
            .expect("a listener publishing must not deadlock the dispatcher");
        dispatcher.drain().await;

        assert_eq!(onward.what_it_heard(), vec!["board.pinned"]);
    }

    #[tokio::test]
    async fn what_a_listener_hears_keeps_the_conversation_it_arrived_in() {
        struct Remembering {
            seen: Mutex<Vec<String>>,
        }

        #[async_trait]
        impl Listener for Remembering {
            fn named(&self) -> ListenerName {
                ListenerName::parse("remembering").expect("a plain name is fine")
            }

            fn listens_to(&self) -> Vec<Subscription> {
                vec![Subscription::parse("#").expect("a plain pattern is fine")]
            }

            async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
                self.seen
                    .lock()
                    .expect("seen lock poisoned")
                    .push(message.conversation.to_string());

                Ok(())
            }
        }

        let dispatcher = a_workbench();
        let listener = Arc::new(Remembering {
            seen: Mutex::new(Vec::new()),
        });
        dispatcher.listen(listener.clone());
        let opening = saying("piece.captured");
        let conversation = opening.conversation.to_string();

        dispatcher
            .publish(opening.clone())
            .await
            .expect("publishing should succeed");
        dispatcher
            .publish(opening.answering(
                RoutingKey::parse("board.pinned").expect("a plain key is fine"),
                json!({}),
                at(1_001),
            ))
            .await
            .expect("publishing should succeed");

        assert_eq!(
            listener.seen.lock().expect("seen lock poisoned").as_slice(),
            [conversation.clone(), conversation],
            "both hops must be traceable to the same request"
        );
    }
}
