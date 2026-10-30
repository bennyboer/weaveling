use async_trait::async_trait;
use eventsourcing::Agent;
use live::{About, Broadcaster, Refused, Subscribed, Topic};

use crate::NoticeChanges;

pub struct NotificationsTopic {
    broadcaster: Broadcaster<NoticeChanges>,
}

impl NotificationsTopic {
    pub fn new(broadcaster: Broadcaster<NoticeChanges>) -> Self {
        Self { broadcaster }
    }
}

#[async_trait]
impl Topic for NotificationsTopic {
    fn named(&self) -> &'static str {
        "notifications"
    }

    async fn subscribe(&self, agent: &Agent, about: &About) -> Result<Subscribed, Refused> {
        match (agent, about.get("mine").map(String::as_str)) {
            (Agent::Account(account), Some("true")) => Ok(Subscribed::hearing(
                self.broadcaster.hear(NoticeChanges(account.clone())),
            )),
            _ => Err(Refused::NotThere),
        }
    }
}
