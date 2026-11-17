use std::collections::HashMap;
use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;

use crate::contents::Block;
use crate::link::{ProjectLink, SceneLink, SectionLink};

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("the catalog of the book could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

/// The book as its outline holds it: its title, and its sections in reading order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookOutline {
    pub title: String,
    pub sections: Vec<OutlinedSection>,
}

/// A section, how deep it is nested, and the scenes it holds in their order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlinedSection {
    pub section: SectionLink,
    pub depth: u8,
    pub title: String,
    pub scenes: Vec<SceneLink>,
}

/// The outline of each project's book, kept by the readings feature itself.
#[async_trait]
pub trait OutlineCatalog: Send + Sync {
    async fn outline_of(&self, project: &ProjectLink) -> Result<Option<BookOutline>, CatalogError>;
}

/// The text of each scene, kept by the readings feature itself.
/// A scene whose text is not kept yet is missing from what it answers.
#[async_trait]
pub trait SceneTextCatalog: Send + Sync {
    async fn texts_of(
        &self,
        scenes: &[SceneLink],
    ) -> Result<HashMap<SceneLink, Vec<Block>>, CatalogError>;
}
