use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use projects_core::{Project, ProjectId, ProjectStore, StoreError};

type Projects = HashMap<ProjectId, Project>;

#[derive(Debug, Default)]
pub struct InMemoryProjectStore {
    projects: RwLock<Projects>,
}

impl InMemoryProjectStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Projects> {
        self.projects.read().expect("project store lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Projects> {
        self.projects.write().expect("project store lock poisoned")
    }
}

#[async_trait]
impl ProjectStore for InMemoryProjectStore {
    async fn create(&self, project: Project) -> Result<(), StoreError> {
        let mut projects = self.write();

        if projects.contains_key(&project.id()) {
            return Err(StoreError::Conflict(project.id()));
        }

        projects.insert(project.id(), project);

        Ok(())
    }

    async fn get(&self, id: ProjectId) -> Result<Project, StoreError> {
        self.read()
            .get(&id)
            .cloned()
            .ok_or(StoreError::NotFound(id))
    }

    async fn list(&self) -> Result<Vec<Project>, StoreError> {
        let mut projects: Vec<Project> = self.read().values().cloned().collect();
        projects.sort_by_key(Project::id);

        Ok(projects)
    }

    async fn update(&self, project: Project) -> Result<(), StoreError> {
        let mut projects = self.write();

        if !projects.contains_key(&project.id()) {
            return Err(StoreError::NotFound(project.id()));
        }

        projects.insert(project.id(), project);

        Ok(())
    }

    async fn delete(&self, id: ProjectId) -> Result<(), StoreError> {
        self.write()
            .remove(&id)
            .map(|_| ())
            .ok_or(StoreError::NotFound(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    crate::suite::conformance_tests!(InMemoryProjectStore::new());
}
