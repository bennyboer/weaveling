use async_trait::async_trait;
use boards_core::BoardsCore;
use chores::Tombstones;
use std::error::Error;
use time::OffsetDateTime;

pub struct BoardTombstones {
    boards: BoardsCore,
}

impl BoardTombstones {
    pub fn of(boards: BoardsCore) -> Self {
        Self { boards }
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
            .boards
            .forget_tombstones_laid_before(before, at_most)
            .await?)
    }
}
