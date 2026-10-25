use std::error::Error;
use std::sync::Arc;

use appearances_core::AppearanceCatalog;
use async_trait::async_trait;
use chores::Tombstones;
use time::OffsetDateTime;

pub struct AppearanceTombstones {
    catalog: Arc<dyn AppearanceCatalog>,
}

impl AppearanceTombstones {
    pub fn of(catalog: Arc<dyn AppearanceCatalog>) -> Self {
        Self { catalog }
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
            .catalog
            .forget_tombstones_laid_before(before, at_most)
            .await?)
    }
}
