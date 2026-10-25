use async_trait::async_trait;
use chores::Tombstones;
use scenes_core::ScenesCore;
use std::error::Error;
use time::OffsetDateTime;

pub struct SceneTombstones {
    scenes: ScenesCore,
}

impl SceneTombstones {
    pub fn of(scenes: ScenesCore) -> Self {
        Self { scenes }
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
            .scenes
            .forget_tombstones_laid_before(before, at_most)
            .await?)
    }
}
