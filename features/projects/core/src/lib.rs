mod catalog;
mod details;
mod id;
mod name;
mod project;
mod service;

#[cfg(test)]
mod project_tests;

pub use catalog::{CatalogError, ProjectCatalog, ProjectSummary};
pub use details::{
    BookLanguage, InvalidBookLanguage, InvalidProjectDescription, InvalidQuotationMarks,
    ProjectDescription, ProjectDetails, QuotationMarks,
};
pub use id::ProjectId;
pub use name::{InvalidProjectName, ProjectName};
pub use project::{KIND, Project, ProjectCommand, ProjectError, ProjectEvent};
pub use service::{ProjectService, ProjectServiceError, RawDetails};
