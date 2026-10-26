use async_trait::async_trait;
use chores::Tombstones;
use outline_core::OutlinesCore;
use std::error::Error;
use time::OffsetDateTime;

pub struct OutlineTombstones {
    outlines: OutlinesCore,
}

impl OutlineTombstones {
    pub fn of(outlines: OutlinesCore) -> Self {
        Self { outlines }
    }
}

#[async_trait]
impl Tombstones for OutlineTombstones {
    async fn forget_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, Box<dyn Error + Send + Sync>> {
        Ok(self
            .outlines
            .forget_tombstones_laid_before(before, at_most)
            .await?)
    }
}
