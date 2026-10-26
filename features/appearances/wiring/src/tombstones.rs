use appearances_core::AppearancesCore;
use async_trait::async_trait;
use chores::Tombstones;
use std::error::Error;
use time::OffsetDateTime;

pub struct AppearanceTombstones {
    appearances: AppearancesCore,
}

impl AppearanceTombstones {
    pub fn of(appearances: AppearancesCore) -> Self {
        Self { appearances }
    }
}

#[async_trait]
impl Tombstones for AppearanceTombstones {
    async fn forget_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, Box<dyn Error + Send + Sync>> {
        Ok(self
            .appearances
            .forget_tombstones_laid_before(before, at_most)
            .await?)
    }
}
