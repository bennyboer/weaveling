use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;

use crate::message::Message;
use crate::routing::{RoutingKey, Subscription};

#[derive(Debug, Error)]
#[error("a listener could not take in {routing}")]
pub struct Unheard {
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
    fn listens_to(&self) -> Subscription;

    async fn hear(&self, message: &Message) -> Result<(), Unheard>;
}

#[async_trait]
pub trait Publisher: Send + Sync {
    async fn publish(&self, message: Message) -> Result<(), Undelivered>;
}

#[async_trait]
pub trait DeadLetters: Send + Sync {
    async fn refused(&self, message: &Message, why: Unheard);
}

impl Unheard {
    pub fn because(routing: RoutingKey, reason: impl Error + Send + Sync + 'static) -> Self {
        Self {
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
    async fn refused(&self, message: &Message, why: Unheard) {
        tracing::error!(
            message = %message.id,
            conversation = %message.conversation,
            routing = %message.routing,
            error = %why,
            "a listener refused a message and there is nowhere to retry it yet"
        );
    }
}
