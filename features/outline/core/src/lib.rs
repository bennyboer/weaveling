mod catalog;
mod id;
mod outline;
mod service;
mod title;

#[cfg(test)]
mod outline_tests;

pub use catalog::{CatalogError, OutlineCatalog, OutlineSummary};
pub use id::{OutlineId, SectionId};
pub use outline::{
    Attachment, IdeaLink, KIND, Outline, OutlineCommand, OutlineError, OutlineEvent, PlacedSection,
    ProjectLink, SceneLink,
};
pub use service::{Added, Open, OutlineService, OutlineServiceError};
pub use title::{InvalidSectionTitle, SectionTitle};
