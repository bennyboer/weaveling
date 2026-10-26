use std::error::Error;
use std::time::Duration;

use async_trait::async_trait;
use chores::Chore;
use invitations_core::InvitationsCore;

const AT_MOST_EACH_ROUND: usize = 100;

pub struct RetireDueInvitations {
    invitations: InvitationsCore,
}

impl RetireDueInvitations {
    pub fn new(invitations: InvitationsCore) -> Self {
        Self { invitations }
    }
}

#[async_trait]
impl Chore for RetireDueInvitations {
    fn named(&self) -> &'static str {
        "retire-due-invitations"
    }

    fn every(&self) -> Duration {
        Duration::from_secs(60 * 60)
    }

    async fn carry_out(&self) -> Result<u64, Box<dyn Error + Send + Sync>> {
        let retired = self.invitations.retire_due(AT_MOST_EACH_ROUND).await?;

        Ok(u64::try_from(retired)?)
    }
}
