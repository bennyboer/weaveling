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
    KIND, Outline, OutlineCommand, OutlineError, OutlineEvent, PieceLink, PlacedSection,
    ProjectLink,
};
pub use service::{Added, Open, OutlineService, OutlineServiceError};
pub use title::{InvalidSectionTitle, SectionTitle};
