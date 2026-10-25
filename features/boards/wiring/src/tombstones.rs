use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use boards_core::BoardCatalog;
use chores::Tombstones;
use time::OffsetDateTime;

pub struct BoardTombstones {
    catalog: Arc<dyn BoardCatalog>,
}

impl BoardTombstones {
    pub fn of(catalog: Arc<dyn BoardCatalog>) -> Self {
        Self { catalog }
    }
}

#[async_trait]
impl Tombstones for BoardTombstones {
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
