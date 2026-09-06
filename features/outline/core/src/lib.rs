mod id;
mod outline;
mod title;

#[cfg(test)]
mod outline_tests;

pub use id::{OutlineId, SectionId};
pub use outline::{
    KIND, Outline, OutlineCommand, OutlineError, OutlineEvent, PieceLink, PlacedSection,
    ProjectLink,
};
pub use title::{InvalidSectionTitle, SectionTitle};
