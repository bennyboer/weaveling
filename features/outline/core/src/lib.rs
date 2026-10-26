mod catalog;
mod error;
mod id;
mod outline;
mod outlines;
mod service;
mod title;

#[cfg(test)]
mod outline_tests;

pub use catalog::{CatalogError, OutlineCatalog, OutlineSummary};
pub use error::OutlinesError;
pub use id::{OutlineId, SectionId};
pub use outline::{
    Attachment, IdeaLink, KIND, Outline, OutlineCommand, OutlineError, OutlineEvent, PlacedSection,
    ProjectLink, SceneLink,
};
pub use outlines::OutlinesCore;
pub use service::{Added, Changed, Open};
pub use title::{InvalidSectionTitle, SectionTitle};
