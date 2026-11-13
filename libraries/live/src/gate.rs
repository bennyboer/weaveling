use std::error::Error;
use std::fmt::Display;
use std::sync::Arc;

use async_trait::async_trait;
use authorizing::AccessWatch;
use eventsourcing::Agent;

use crate::{About, Broadcaster, Channel, Refused, Subscribed, Topic};

/// Decides who may follow what a topic is about.
/// The thing is given as text, for example a project's id or a board's id.
#[async_trait]
pub trait Gate: Send + Sync {
    /// Whether `agent` may follow `about` now.
    /// Returns an error when that cannot be decided; an error means neither yes nor no.
    async fn admits(
        &self,
        agent: &Agent,
        about: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// Returns a watch that wakes when access to `about` may have changed, so the gate is asked again.
    /// By default it never wakes.
    async fn watch(&self, _about: &str) -> AccessWatch {
        AccessWatch::still()
    }
}

/// A follower that a gate let in.
/// It is kept so the gate can be asked again whenever its watch wakes.
pub struct Admission {
    watch: AccessWatch,
    gate: Arc<dyn Gate>,
    agent: Agent,
    about: String,
}

pub(crate) enum Access {
    Kept,
    Lost,
}

/// Asks `gate` whether `agent` may follow `about`.
/// It starts watching first, so no access change is missed in between.
/// The result is one of: let in, refused as not there, or refused as failed when the gate cannot decide.
pub async fn admitted(
    gate: Arc<dyn Gate>,
    agent: &Agent,
    about: &str,
) -> Result<Admission, Refused> {
    let watch = gate.watch(about).await;

    match gate.admits(agent, about).await {
        Ok(true) => Ok(Admission {
            watch,
            gate,
            agent: agent.clone(),
            about: about.to_owned(),
        }),
        Ok(false) => Err(Refused::NotThere),
        Err(_) => Err(Refused::Failed),
    }
}

impl Admission {
    pub(crate) async fn changed(&mut self) {
        self.watch.changed().await;
    }

    pub(crate) async fn asked_again(&self) -> Access {
        match self.gate.admits(&self.agent, &self.about).await {
            Ok(true) => Access::Kept,
            _ => Access::Lost,
        }
    }
}

/// The topic of a feature that only needs a gate.
/// Each subscription listens to one channel, which `channel_of` derives from what the subscription is about.
/// It listens only while the gate lets it in.
pub struct GatedTopic<C> {
    named: &'static str,
    broadcaster: Broadcaster<C>,
    channel_of: fn(&About) -> Option<C>,
    gate: Arc<dyn Gate>,
}

impl<C: Channel> GatedTopic<C> {
    pub fn new(
        named: &'static str,
        broadcaster: Broadcaster<C>,
        channel_of: fn(&About) -> Option<C>,
        gate: Arc<dyn Gate>,
    ) -> Self {
        Self {
            named,
            broadcaster,
            channel_of,
            gate,
        }
    }
}

#[async_trait]
impl<C: Channel + Display> Topic for GatedTopic<C> {
    fn named(&self) -> &'static str {
        self.named
    }

    async fn subscribe(&self, agent: &Agent, about: &About) -> Result<Subscribed, Refused> {
        let Some(channel) = (self.channel_of)(about) else {
            return Err(Refused::NotThere);
        };
        let about = channel.to_string();
        let hearing = self.broadcaster.hear(channel);
        let admission = admitted(self.gate.clone(), agent, &about).await?;

        Ok(Subscribed::hearing(hearing).while_admitted(admission))
    }
}
