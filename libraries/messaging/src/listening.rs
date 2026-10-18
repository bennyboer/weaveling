use std::error::Error;
use std::fmt::{self, Display, Formatter};

use async_trait::async_trait;
use thiserror::Error;

use crate::message::Message;
use crate::routing::{RoutingKey, Subscription};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ListenerName(String);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidListenerName {
    #[error("a listener must be named")]
    Empty,
    #[error("a listener name may hold only lowercase letters, digits and hyphens")]
    Malformed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Delivery {
    Kept,
    Fleeting,
}

#[derive(Debug, Error)]
#[error("{listener} could not take in {routing}")]
pub struct NotHandled {
    pub listener: ListenerName,
    pub routing: RoutingKey,
    #[source]
    pub because: Box<dyn Error + Send + Sync>,
}

#[derive(Debug, Error)]
#[error("{routing} could not be handed over")]
pub struct Undelivered {
    pub routing: RoutingKey,
    #[source]
    pub because: Box<dyn Error + Send + Sync>,
}

#[async_trait]
pub trait Listener: Send + Sync {
    fn named(&self) -> ListenerName;

    fn listens_to(&self) -> Vec<Subscription>;

    fn when_refused(&self) -> &'static str;

    fn hears(&self, routing: &RoutingKey) -> bool {
        self.listens_to().iter().any(|to| to.covers(routing))
    }

    fn delivery(&self) -> Delivery {
        Delivery::Kept
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled>;
}

pub type Failure = Box<dyn Error + Send + Sync>;

#[async_trait]
pub trait Handler: Send + Sync {
    const NAME: &'static str;
    const LISTENS_TO: &'static [&'static str];
    const WHEN_REFUSED: &'static str;

    async fn take_in(&self, message: &Message) -> Result<(), Failure>;
}

#[async_trait]
impl<H: Handler> Listener for H {
    fn named(&self) -> ListenerName {
        ListenerName::parse(H::NAME).expect("a handler's name is written at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        H::LISTENS_TO
            .iter()
            .map(|pattern| {
                Subscription::parse(pattern)
                    .expect("a handler's patterns are written at compile time")
            })
            .collect()
    }

    fn when_refused(&self) -> &'static str {
        H::WHEN_REFUSED
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        self.take_in(message).await.map_err(|because| NotHandled {
            listener: self.named(),
            routing: message.routing.clone(),
            because,
        })
    }
}

#[async_trait]
pub trait Publisher: Send + Sync {
    async fn publish(&self, message: Message) -> Result<(), Undelivered>;
}

impl ListenerName {
    pub fn parse(name: &str) -> Result<Self, InvalidListenerName> {
        if name.is_empty() {
            return Err(InvalidListenerName::Empty);
        }

        if !name.chars().all(speakable) {
            return Err(InvalidListenerName::Malformed);
        }

        Ok(Self(name.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn speakable(character: char) -> bool {
    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
}

impl Display for ListenerName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl NotHandled {
    pub fn because(
        listener: ListenerName,
        routing: RoutingKey,
        reason: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            listener,
            routing,
            because: Box::new(reason),
        }
    }
}

impl Undelivered {
    pub fn because(routing: RoutingKey, reason: impl Error + Send + Sync + 'static) -> Self {
        Self {
            routing,
            because: Box::new(reason),
        }
    }
}

#[async_trait::async_trait]
pub trait Notifications: Send {
    async fn wait(&mut self);
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use time::OffsetDateTime;

    use super::*;

    #[test]
    fn a_name_reads_back_as_it_was_written() {
        let name = ListenerName::parse("ideas-catalog").expect("a plain name is fine");

        assert_eq!(name.as_str(), "ideas-catalog");
        assert_eq!(name.to_string(), "ideas-catalog");
    }

    #[test]
    fn a_nameless_listener_is_refused() {
        assert_eq!(ListenerName::parse(""), Err(InvalidListenerName::Empty));
    }

    #[test]
    fn a_name_that_would_not_survive_a_queue_is_refused() {
        for unspeakable in ["Ideas", "ideas catalog", "ideas.catalog", "ideas_catalog"] {
            assert_eq!(
                ListenerName::parse(unspeakable),
                Err(InvalidListenerName::Malformed),
                "{unspeakable} should not become a queue name"
            );
        }
    }

    #[test]
    fn digits_and_hyphens_are_welcome() {
        assert!(ListenerName::parse("board-live-v2").is_ok());
    }

    struct Refusing;

    #[derive(Debug, Error)]
    #[error("the loom is jammed")]
    struct Jammed;

    #[async_trait]
    impl Handler for Refusing {
        const NAME: &'static str = "refuse-everything";
        const LISTENS_TO: &'static [&'static str] = &["idea.#", "project.deleted"];
        const WHEN_REFUSED: &'static str = "Nothing was taken in.";

        async fn take_in(&self, _message: &Message) -> Result<(), Failure> {
            Err(Box::new(Jammed))
        }
    }

    #[test]
    fn a_handler_listens_as_it_declares() {
        let listener: &dyn Listener = &Refusing;

        assert_eq!(listener.named().as_str(), "refuse-everything");
        assert_eq!(listener.when_refused(), "Nothing was taken in.");
        assert!(listener.hears(&RoutingKey::parse("idea.captured").expect("a plain key")));
        assert!(listener.hears(&RoutingKey::parse("project.deleted").expect("a plain key")));
        assert!(!listener.hears(&RoutingKey::parse("project.started").expect("a plain key")));
        assert_eq!(listener.delivery(), Delivery::Kept);
    }

    #[tokio::test]
    async fn a_handler_that_fails_says_who_failed_at_what_and_why() {
        let routing = RoutingKey::parse("idea.captured").expect("a plain key");
        let message = Message::opening(routing.clone(), json!({}), OffsetDateTime::UNIX_EPOCH);

        let refused = Refusing
            .handle(&message)
            .await
            .expect_err("the handler refuses everything");

        assert_eq!(refused.listener.as_str(), "refuse-everything");
        assert_eq!(refused.routing, routing);
        assert_eq!(refused.because.to_string(), "the loom is jammed");
    }
}
