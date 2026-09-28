mod catalog;
mod id;
mod name;
mod project;
mod service;

#[cfg(test)]
mod project_tests;

pub use catalog::{CatalogError, ProjectCatalog, ProjectSummary};
pub use id::ProjectId;
pub use name::{InvalidProjectName, ProjectName};
pub use project::{KIND, Project, ProjectCommand, ProjectError, ProjectEvent};
pub use service::{ProjectService, ProjectServiceError};
