use std::error::Error;

use async_trait::async_trait;
use chores::Tombstones;
use invitations_core::InvitationsCore;
use time::OffsetDateTime;

pub struct InvitationTombstones {
    invitations: InvitationsCore,
}

impl InvitationTombstones {
    pub fn of(invitations: InvitationsCore) -> Self {
        Self { invitations }
    }
}

#[async_trait]
impl Tombstones for InvitationTombstones {
    async fn forget_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, Box<dyn Error + Send + Sync>> {
        Ok(self
            .invitations
            .forget_tombstones_laid_before(before, at_most)
            .await?)
    }
}
