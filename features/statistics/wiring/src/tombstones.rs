use async_trait::async_trait;
use chores::Tombstones;
use statistics_core::StatisticsCore;
use std::error::Error;
use time::OffsetDateTime;

pub struct StatisticsTombstones {
    statistics: StatisticsCore,
}

impl StatisticsTombstones {
    pub fn of(statistics: StatisticsCore) -> Self {
        Self { statistics }
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
            .statistics
            .forget_tombstones_laid_before(before, at_most)
            .await?)
    }
}
