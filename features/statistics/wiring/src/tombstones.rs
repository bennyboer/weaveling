use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use chores::Tombstones;
use statistics_core::WordCountCatalog;
use time::OffsetDateTime;

pub struct StatisticsTombstones {
    catalog: Arc<dyn WordCountCatalog>,
}

impl StatisticsTombstones {
    pub fn of(catalog: Arc<dyn WordCountCatalog>) -> Self {
        Self { catalog }
    }
}

#[async_trait]
impl Tombstones for StatisticsTombstones {
    async fn forget_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, Box<dyn Error + Send + Sync>> {
        Ok(self
            .catalog
            .forget_tombstones_laid_before(before, at_most)
            .await?)
    }
}
