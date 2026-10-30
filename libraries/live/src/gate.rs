use std::error::Error;
use std::fmt::Display;
use std::sync::Arc;

use async_trait::async_trait;
use authorizing::AccessWatch;
use eventsourcing::Agent;

use crate::{About, Broadcaster, Channel, Refused, Subscribed, Topic};

/// Decides who may follow what a topic is about, told as text: a project's id, a board's.
#[async_trait]
pub trait Gate: Send + Sync {
    /// Whether `agent` may follow `about` now; an error when that cannot be told, which is no answer either way.
    async fn admits(
        &self,
        agent: &Agent,
        about: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// What wakes when who may follow `about` may have changed, so the gate is asked again; by default nothing ever does.
    async fn watch(&self, _about: &str) -> AccessWatch {
        AccessWatch::still()
    }
}

/// A follower a gate let in, kept to ask the gate again whenever its watch wakes.
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

/// Asks `gate` whether `agent` may follow `about`, watching first so that no change of access slips in between: let in, refused as not there, or refused as failed when the gate cannot tell.
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
    pub(crate) async fn rechecked(&mut self) -> Access {
        self.watch.changed().await;
        match self.gate.admits(&self.agent, &self.about).await {
            Ok(true) => Access::Kept,
            _ => Access::Lost,
        }
    }
}

/// The topic of a feature that only asks a gate: each subscription hears the one channel `channel_of` reads from what it is about, while the gate lets it in.
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
        let admission = admitted(self.gate.clone(), agent, &channel.to_string()).await?;

        Ok(Subscribed::hearing(self.broadcaster.hear(channel)).while_admitted(admission))
    }
}
