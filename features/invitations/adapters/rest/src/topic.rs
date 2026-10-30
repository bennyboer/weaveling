use async_trait::async_trait;
use eventsourcing::Agent;
use invitations_core::InvitationsCore;
use live::{Asked, Broadcaster, Refused, Subscribed, Topic};

use crate::InvitationChanges;

pub struct InvitationsTopic {
    invitations: InvitationsCore,
    broadcaster: Broadcaster<InvitationChanges>,
}

impl InvitationsTopic {
    pub fn new(invitations: InvitationsCore, broadcaster: Broadcaster<InvitationChanges>) -> Self {
        Self {
            invitations,
            broadcaster,
        }
    }
}

#[async_trait]
impl Topic for InvitationsTopic {
    fn named(&self) -> &'static str {
        "invitations"
    }

    async fn subscribe(&self, agent: &Agent, about: &Asked) -> Result<Subscribed, Refused> {
        if about.get("mine").map(String::as_str) != Some("true") {
            return Err(Refused::NotThere);
        }

        match self.invitations.address_of(agent).await {
            Ok(Some(address)) => Ok(Subscribed::hearing(
                self.broadcaster.hear(InvitationChanges(address)),
            )),
            Ok(None) => Err(Refused::NotThere),
            Err(_) => Err(Refused::Failed),
        }
    }
}
