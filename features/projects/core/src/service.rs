use std::sync::Arc;

use clock::Clock;
use thiserror::Error;

use crate::{
    InvalidProjectId, InvalidProjectName, Project, ProjectId, ProjectName, ProjectStore, StoreError,
};

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error(transparent)]
    InvalidId(#[from] InvalidProjectId),
    #[error(transparent)]
    InvalidName(#[from] InvalidProjectName),
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(Clone)]
pub struct ProjectService {
    store: Arc<dyn ProjectStore>,
    clock: Arc<dyn Clock>,
}

impl ProjectService {
    pub fn new(store: Arc<dyn ProjectStore>, clock: Arc<dyn Clock>) -> Self {
        Self { store, clock }
    }

    pub async fn create(&self, name: &str) -> Result<Project, ProjectError> {
        let name = ProjectName::new(name)?;
        let project = Project::new(name, self.clock.now());

        self.store.create(project.clone()).await?;

        Ok(project)
    }

    pub async fn get(&self, id: &str) -> Result<Project, ProjectError> {
        let id: ProjectId = id.parse()?;

        Ok(self.store.get(id).await?)
    }

    pub async fn list(&self) -> Result<Vec<Project>, ProjectError> {
        Ok(self.store.list().await?)
    }

    pub async fn rename(&self, id: &str, name: &str) -> Result<Project, ProjectError> {
        let id: ProjectId = id.parse()?;
        let name = ProjectName::new(name)?;

        let mut project = self.store.get(id).await?;
        project.rename(name, self.clock.now());
        self.store.update(project.clone()).await?;

        Ok(project)
    }

    pub async fn delete(&self, id: &str) -> Result<(), ProjectError> {
        let id: ProjectId = id.parse()?;

        Ok(self.store.delete(id).await?)
    }
}
