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

    fn listens_to(&self) -> Subscription;

    fn delivery(&self) -> Delivery {
        Delivery::Kept
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled>;
}

#[async_trait]
pub trait Publisher: Send + Sync {
    async fn publish(&self, message: Message) -> Result<(), Undelivered>;
}

#[async_trait]
pub trait DeadLetters: Send + Sync {
    async fn refused(&self, message: &Message, why: NotHandled);
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

pub struct Logged;

#[async_trait]
impl DeadLetters for Logged {
    async fn refused(&self, message: &Message, why: NotHandled) {
        tracing::error!(
            listener = %why.listener,
            message = %message.id,
            conversation = %message.conversation,
            routing = %message.routing,
            error = %why,
            "a listener refused a message and there is nowhere to retry it yet"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_reads_back_as_it_was_written() {
        let name = ListenerName::parse("pieces-catalog").expect("a plain name is fine");

        assert_eq!(name.as_str(), "pieces-catalog");
        assert_eq!(name.to_string(), "pieces-catalog");
    }

    #[test]
    fn a_nameless_listener_is_refused() {
        assert_eq!(ListenerName::parse(""), Err(InvalidListenerName::Empty));
    }

    #[test]
    fn a_name_that_would_not_survive_a_queue_is_refused() {
        for unspeakable in [
            "Pieces",
            "pieces catalog",
            "pieces.catalog",
            "pieces_catalog",
        ] {
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
}
