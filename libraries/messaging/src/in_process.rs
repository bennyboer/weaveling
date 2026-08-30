use std::sync::{Arc, RwLock};

use async_trait::async_trait;

use crate::listening::{DeadLetters, Delivery, Listener, Logged, Publisher, Undelivered};
use crate::message::Message;

pub struct InProcessDispatcher {
    listeners: RwLock<Vec<Arc<dyn Listener>>>,
    dead_letters: Arc<dyn DeadLetters>,
}

impl Default for InProcessDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl InProcessDispatcher {
    pub fn new() -> Self {
        Self::dead_lettering_to(Arc::new(Logged))
    }

    pub fn dead_lettering_to(dead_letters: Arc<dyn DeadLetters>) -> Self {
        Self {
            listeners: RwLock::new(Vec::new()),
            dead_letters,
        }
    }

    pub fn listen(&self, listener: Arc<dyn Listener>) {
        self.listeners
            .write()
            .expect("messaging lock poisoned")
            .push(listener);
    }

    fn interested_in(&self, message: &Message) -> Vec<Arc<dyn Listener>> {
        self.listeners
            .read()
            .expect("messaging lock poisoned")
            .iter()
            .filter(|listener| listener.listens_to().covers(&message.routing))
            .cloned()
            .collect()
    }
}

#[async_trait]
impl Publisher for InProcessDispatcher {
    async fn publish(&self, message: Message) -> Result<(), Undelivered> {
        for listener in self.interested_in(&message) {
            if let Err(refused) = listener.hear(&message).await {
                match listener.delivery() {
                    // TODO local mode must not dead letter a Kept refusal either:
                    // there is nobody to retry it, so it has to reach the author
                    Delivery::Kept => self.dead_letters.refused(&message, refused).await,
                    Delivery::Fleeting => tracing::debug!(
                        listener = %refused.listener,
                        routing = %message.routing,
                        error = %refused,
                        "a fleeting listener let a message go by"
                    ),
                }
            }
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

    use super::*;
    use crate::listening::{ListenerName, Unheard};
    use crate::routing::{RoutingKey, Subscription};

    #[derive(Debug, Error)]
    #[error("this listener always refuses")]
    struct Refused;

    struct Overheard {
        name: ListenerName,
        subscription: Subscription,
        delivery: Delivery,
        heard: Mutex<Vec<RoutingKey>>,
        refuses: bool,
    }

    #[async_trait]
    impl Listener for Overheard {
        fn named(&self) -> ListenerName {
            self.name.clone()
        }

        fn listens_to(&self) -> Subscription {
            self.subscription.clone()
        }

        fn delivery(&self) -> Delivery {
            self.delivery
        }

        async fn hear(&self, message: &Message) -> Result<(), Unheard> {
            self.heard
                .lock()
                .expect("heard lock poisoned")
                .push(message.routing.clone());

            if self.refuses {
                return Err(Unheard::because(
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
                subscription: Subscription::parse(to).expect("a plain pattern is fine"),
                delivery,
                heard: Mutex::new(Vec::new()),
                refuses,
            })
        }

        fn listening(to: &str) -> Arc<Self> {
            Self::named("listening", to, Delivery::Kept, false)
        }

        fn refusing(to: &str) -> Arc<Self> {
            Self::named("refusing", to, Delivery::Kept, true)
        }

        fn refusing_fleetingly(to: &str) -> Arc<Self> {
            Self::named("fleeting", to, Delivery::Fleeting, true)
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
        let dispatcher = InProcessDispatcher::new();
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
        let dispatcher = InProcessDispatcher::new();
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
        let dispatcher = InProcessDispatcher::new();
        let exact = Overheard::listening("piece.captured");
        let wildcard = Overheard::listening("piece.*");
        let everything = Overheard::listening("#");
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
    async fn a_message_nobody_wants_is_not_a_failure() {
        let dispatcher = InProcessDispatcher::new();

        dispatcher
            .publish(saying("piece.captured"))
            .await
            .expect("nobody listening is not an error, it is just quiet");
    }

    #[derive(Default)]
    struct Kept {
        refused: Mutex<Vec<(String, String)>>,
    }

    #[async_trait]
    impl DeadLetters for Kept {
        async fn refused(&self, message: &Message, why: Unheard) {
            self.refused
                .lock()
                .expect("dead letters lock poisoned")
                .push((why.listener.to_string(), message.routing.to_string()));
        }
    }

    impl Kept {
        fn what_it_kept(&self) -> Vec<(String, String)> {
            self.refused
                .lock()
                .expect("dead letters lock poisoned")
                .clone()
        }
    }

    #[tokio::test]
    async fn one_listener_refusing_does_not_rob_the_others() {
        let dead_letters = Arc::new(Kept::default());
        let dispatcher = InProcessDispatcher::dead_lettering_to(dead_letters.clone());
        let refusing = Overheard::refusing("piece.captured");
        let willing = Overheard::listening("piece.captured");
        dispatcher.listen(refusing.clone());
        dispatcher.listen(willing.clone());

        dispatcher
            .publish(saying("piece.captured"))
            .await
            .expect("a publisher never learns what listeners made of it");

        assert_eq!(
            willing.what_it_heard().len(),
            1,
            "the second still heard it"
        );
        assert_eq!(
            dead_letters.what_it_kept(),
            [("refusing".to_owned(), "piece.captured".to_owned())],
            "the refusal is not lost, it is set aside for whoever retries later"
        );
    }

    #[tokio::test]
    async fn dead_letters_learn_which_listener_refused() {
        let dead_letters = Arc::new(Kept::default());
        let dispatcher = InProcessDispatcher::dead_lettering_to(dead_letters.clone());
        dispatcher.listen(Overheard::named(
            "pieces-catalog",
            "piece.captured",
            Delivery::Kept,
            true,
        ));
        dispatcher.listen(Overheard::named(
            "deletion-saga",
            "piece.captured",
            Delivery::Kept,
            true,
        ));

        dispatcher
            .publish(saying("piece.captured"))
            .await
            .expect("publishing should succeed");

        assert_eq!(
            dead_letters
                .what_it_kept()
                .into_iter()
                .map(|(listener, _)| listener)
                .collect::<Vec<_>>(),
            ["pieces-catalog", "deletion-saga"],
            "over a broker each listener has its own dead queue, so in process the sink must be told"
        );
    }

    #[tokio::test]
    async fn a_fleeting_listener_refusing_is_not_dead_lettered() {
        let dead_letters = Arc::new(Kept::default());
        let dispatcher = InProcessDispatcher::dead_lettering_to(dead_letters.clone());
        let fleeting = Overheard::refusing_fleetingly("piece.captured");
        dispatcher.listen(fleeting.clone());

        dispatcher
            .publish(saying("piece.captured"))
            .await
            .expect("publishing should succeed");

        assert_eq!(fleeting.what_it_heard().len(), 1, "it was still offered");
        assert!(
            dead_letters.what_it_kept().is_empty(),
            "nothing will replay it, so setting it aside would only grow a pile nobody drains"
        );
    }

    #[tokio::test]
    async fn a_listener_is_kept_unless_it_says_otherwise() {
        struct Quiet;

        #[async_trait]
        impl Listener for Quiet {
            fn named(&self) -> ListenerName {
                ListenerName::parse("quiet").expect("a plain name is fine")
            }

            fn listens_to(&self) -> Subscription {
                Subscription::parse("#").expect("a plain pattern is fine")
            }

            async fn hear(&self, _message: &Message) -> Result<(), Unheard> {
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
        let dead_letters = Arc::new(Kept::default());
        let dispatcher = InProcessDispatcher::dead_lettering_to(dead_letters.clone());
        dispatcher.listen(Overheard::listening("piece.captured"));

        dispatcher
            .publish(saying("piece.captured"))
            .await
            .expect("publishing should succeed");

        assert!(dead_letters.what_it_kept().is_empty());
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

            fn listens_to(&self) -> Subscription {
                Subscription::parse("piece.captured").expect("a plain pattern is fine")
            }

            async fn hear(&self, message: &Message) -> Result<(), Unheard> {
                let answer = message.answering(
                    RoutingKey::parse("board.pinned").expect("a plain key is fine"),
                    json!({}),
                    at(1_001),
                );
                let _ = self.dispatcher.publish(answer).await;

                Ok(())
            }
        }

        let dispatcher = Arc::new(InProcessDispatcher::new());
        let onward = Overheard::listening("board.pinned");
        dispatcher.listen(onward.clone());
        dispatcher.listen(Arc::new(Echoing {
            dispatcher: dispatcher.clone(),
        }));

        dispatcher
            .publish(saying("piece.captured"))
            .await
            .expect("a listener publishing must not deadlock the dispatcher");

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

            fn listens_to(&self) -> Subscription {
                Subscription::parse("#").expect("a plain pattern is fine")
            }

            async fn hear(&self, message: &Message) -> Result<(), Unheard> {
                self.seen
                    .lock()
                    .expect("seen lock poisoned")
                    .push(message.conversation.to_string());

                Ok(())
            }
        }

        let dispatcher = InProcessDispatcher::new();
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
