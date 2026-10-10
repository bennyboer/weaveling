use std::collections::hash_map::RandomState;
use std::env;
use std::hash::{BuildHasher, Hasher};
use std::sync::Arc;

use async_trait::async_trait;
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use thiserror::Error;

pub const WEAVELING_FLAKY: &str = "WEAVELING_FLAKY";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flakiness(f64);

#[derive(Debug, Error)]
#[error("{WEAVELING_FLAKY} is the share of messages refused on purpose, from 0 to 1, not {0:?}")]
pub struct Unflaky(String);

#[derive(Debug, Error)]
#[error("refused on purpose, because {WEAVELING_FLAKY} is set")]
struct OnPurpose;

pub struct Flaky {
    listener: Arc<dyn Listener>,
    flakiness: Flakiness,
}

impl Flakiness {
    pub fn from_environment() -> Result<Option<Self>, Unflaky> {
        env::var(WEAVELING_FLAKY)
            .ok()
            .map(|told| Self::parse(&told))
            .transpose()
    }

    pub fn parse(told: &str) -> Result<Self, Unflaky> {
        match told.trim().parse::<f64>() {
            Ok(share) if (0.0..=1.0).contains(&share) => Ok(Self(share)),
            _ => Err(Unflaky(told.to_owned())),
        }
    }

    fn strikes(self) -> bool {
        let roll = RandomState::new().build_hasher().finish() as f64 / u64::MAX as f64;

        roll < self.0
    }
}

impl Flaky {
    pub fn wrapping(listener: Arc<dyn Listener>, flakiness: Flakiness) -> Arc<dyn Listener> {
        Arc::new(Self {
            listener,
            flakiness,
        })
    }
}

#[async_trait]
impl Listener for Flaky {
    fn named(&self) -> ListenerName {
        self.listener.named()
    }

    fn listens_to(&self) -> Vec<Subscription> {
        self.listener.listens_to()
    }

    fn when_refused(&self) -> &'static str {
        self.listener.when_refused()
    }

    fn delivery(&self) -> Delivery {
        self.listener.delivery()
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        if self.flakiness.strikes() {
            return Err(NotHandled::because(
                self.named(),
                message.routing.clone(),
                OnPurpose,
            ));
        }

        self.listener.handle(message).await
    }
}
