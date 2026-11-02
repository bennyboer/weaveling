use std::error::Error;

use async_trait::async_trait;
use projects_core::{KIND, ProjectsCore};
use trash::{Purge, Trashed};

pub struct ProjectsPurge {
    projects: ProjectsCore,
}

impl ProjectsPurge {
    pub fn of(projects: ProjectsCore) -> Self {
        Self { projects }
    }
}

#[async_trait]
impl Purge for ProjectsPurge {
    async fn purge(&self, trashed: &Trashed) -> Result<(), Box<dyn Error + Send + Sync>> {
        if trashed.kind != KIND.as_str() {
            return Err(format!("projects keep no {} in their trash", trashed.kind).into());
        }

        Ok(self.projects.purge(&trashed.id).await?)
    }
}
