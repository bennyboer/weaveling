use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chores::Chore;
use clock::Clock;
use invitations_core::InvitationCatalog;

pub struct ForgetDueInvitations {
    catalog: Arc<dyn InvitationCatalog>,
    clock: Arc<dyn Clock>,
}

impl ForgetDueInvitations {
    pub fn new(catalog: Arc<dyn InvitationCatalog>, clock: Arc<dyn Clock>) -> Self {
        Self { catalog, clock }
    }
}

#[async_trait]
impl Chore for ForgetDueInvitations {
    fn named(&self) -> &'static str {
        "forget-due-invitations"
    }

    fn every(&self) -> Duration {
        Duration::from_secs(60 * 60)
    }

    async fn carry_out(&self) -> Result<u64, Box<dyn Error + Send + Sync>> {
        Ok(self.catalog.forget_due(self.clock.now()).await?)
    }
}
