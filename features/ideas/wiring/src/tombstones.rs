use std::error::Error;

use async_trait::async_trait;
use chores::Tombstones;
use ideas_core::IdeasCore;
use time::OffsetDateTime;

pub struct IdeaTombstones {
    ideas: IdeasCore,
}

impl IdeaTombstones {
    pub fn of(ideas: IdeasCore) -> Self {
        Self { ideas }
    }
}

#[async_trait]
impl Tombstones for IdeaTombstones {
    async fn forget_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, Box<dyn Error + Send + Sync>> {
        Ok(self
            .ideas
            .forget_tombstones_laid_before(before, at_most)
            .await?)
    }
}
