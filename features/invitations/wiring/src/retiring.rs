use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chores::Chore;
use clock::Clock;
use invitations_core::{InvitationCatalog, InvitationService};

const AT_MOST_EACH_ROUND: usize = 100;

pub struct RetireDueInvitations {
    catalog: Arc<dyn InvitationCatalog>,
    invitations: InvitationService,
    clock: Arc<dyn Clock>,
}

impl RetireDueInvitations {
    pub fn new(
        catalog: Arc<dyn InvitationCatalog>,
        invitations: InvitationService,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            catalog,
            invitations,
            clock,
        }
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
        let due = self
            .catalog
            .due_for_retiring(self.clock.now(), AT_MOST_EACH_ROUND)
            .await?;
        for invitation in &due {
            self.invitations.retire(invitation).await?;
        }

        Ok(u64::try_from(due.len())?)
    }
}
