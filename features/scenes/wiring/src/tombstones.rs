use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use chores::Tombstones;
use scenes_core::SceneCatalog;
use time::OffsetDateTime;

pub struct SceneTombstones {
    catalog: Arc<dyn SceneCatalog>,
}

impl SceneTombstones {
    pub fn of(catalog: Arc<dyn SceneCatalog>) -> Self {
        Self { catalog }
    }
}

#[async_trait]
impl Tombstones for SceneTombstones {
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
