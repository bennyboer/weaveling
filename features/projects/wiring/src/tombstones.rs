use std::error::Error;

use async_trait::async_trait;
use chores::Tombstones;
use projects_core::ProjectsCore;
use time::OffsetDateTime;

pub struct ProjectTombstones {
    projects: ProjectsCore,
}

impl ProjectTombstones {
    pub fn of(projects: ProjectsCore) -> Self {
        Self { projects }
    }
}

#[async_trait]
impl Tombstones for ProjectTombstones {
    async fn forget_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, Box<dyn Error + Send + Sync>> {
        Ok(self
            .projects
            .forget_tombstones_laid_before(before, at_most)
            .await?)
    }
}
