use async_trait::async_trait;
use thiserror::Error;

use crate::{Project, ProjectId};

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("project {0} was not found")]
    NotFound(ProjectId),
    #[error("project {0} already exists")]
    Conflict(ProjectId),
    #[error("the project store failed: {0}")]
    Backend(#[source] Box<dyn std::error::Error + Send + Sync>),
}

#[async_trait]
pub trait ProjectStore: Send + Sync {
    async fn create(&self, project: Project) -> Result<(), StoreError>;

    async fn get(&self, id: ProjectId) -> Result<Project, StoreError>;

    async fn list(&self) -> Result<Vec<Project>, StoreError>;

    async fn update(&self, project: Project) -> Result<(), StoreError>;

    async fn delete(&self, id: ProjectId) -> Result<(), StoreError>;
}
