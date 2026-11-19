use std::collections::HashMap;
use std::error::Error;

use async_trait::async_trait;
use eventsourcing::{AccountId, Agent};
use thiserror::Error;
use time::OffsetDateTime;
use transacting::Transaction;

use crate::contents::{Block, Contents};
use crate::id::ReadingCopyId;
use crate::link::{ProjectLink, SceneLink, SectionLink};
use crate::reading_copies::Extent;
use crate::reading_copy::{ReadingCopy, Times};
use crate::reading_copy_name::ReadingCopyName;

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("what the readings feature keeps could not be reached")]
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

/// The outline of each project's book and the project's name as its title, kept by the readings feature itself.
/// The outline and the title come from different features, so each is kept at a version of its own.
#[async_trait]
pub trait OutlineCatalog: Send + Sync {
    /// Keeps a book's sections, as the outline told them at a version.
    /// Sections older than the ones kept, or of a deleted project, are not kept.
    /// Returns whether they were.
    async fn keep_outline(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        sections: &[OutlinedSection],
        version: u64,
    ) -> Result<bool, CatalogError>;

    /// Keeps a book's title, as the projects feature named the project at a version.
    /// A title older than the one kept, or of a deleted project, is not kept.
    /// Returns whether it was.
    async fn keep_title(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        title: &str,
        version: u64,
    ) -> Result<bool, CatalogError>;

    /// Forgets a deleted project's book, and lays a tombstone so a late outline or title does not bring it back.
    async fn forget_book(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;

    /// The book's title and sections.
    /// A book with a title but no outline yet has no sections, and one with an outline but no title yet has an empty title.
    /// A book with neither is not there.
    async fn outline_of(&self, project: &ProjectLink) -> Result<Option<BookOutline>, CatalogError>;
}

/// The text of each scene, kept by the readings feature itself.
/// A scene whose text is not kept yet is missing from what it answers.
#[async_trait]
pub trait SceneTextCatalog: Send + Sync {
    /// Keeps a scene's text, read at its prose version.
    /// A text older than the one kept, or of a scene already deleted, is not kept.
    /// Returns whether it was.
    async fn keep(
        &self,
        transaction: &mut Transaction,
        scene: &SceneLink,
        blocks: &[Block],
        version: u64,
    ) -> Result<bool, CatalogError>;

    /// Forgets a deleted scene's text, and lays a tombstone so a late text does not bring it back.
    async fn forget_scene(
        &self,
        transaction: &mut Transaction,
        scene: &SceneLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;

    async fn texts_of(
        &self,
        scenes: &[SceneLink],
    ) -> Result<HashMap<SceneLink, Vec<Block>>, CatalogError>;
}

/// A reading copy as the list of its project's copies keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingCopySummary {
    pub copy: ReadingCopyId,
    pub project: ProjectLink,
    pub name: ReadingCopyName,
    pub made_by: Agent,
    pub made_at: OffsetDateTime,
    pub extent: Extent,
    pub times: Times,
}

impl ReadingCopySummary {
    pub fn of(copy: ReadingCopyId, standing: &ReadingCopy) -> Self {
        Self {
            copy,
            project: standing.project().clone(),
            name: standing.name().clone(),
            made_by: standing.made_by().clone(),
            made_at: standing.made_at(),
            extent: standing.extent(),
            times: standing.times(),
        }
    }
}

/// The reading copies of each project, kept by the readings feature from the copies' own events.
#[async_trait]
pub trait ReadingCopyCatalog: Send + Sync {
    /// Keeps a copy as it stands now, in place of what was kept of it before.
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &ReadingCopySummary,
    ) -> Result<(), CatalogError>;

    /// Forgets a deleted copy.
    async fn forget(
        &self,
        transaction: &mut Transaction,
        copy: &ReadingCopyId,
    ) -> Result<(), CatalogError>;

    /// Every copy of a project, the newest first.
    async fn copies_of(
        &self,
        project: &ProjectLink,
    ) -> Result<Vec<ReadingCopySummary>, CatalogError>;

    /// At most so many copies whose deletion has come by the moment, the earliest due first.
    async fn due_for_deletion(
        &self,
        at: OffsetDateTime,
        at_most: u64,
    ) -> Result<Vec<ReadingCopyId>, CatalogError>;
}

/// The name each account signed up with, kept by the readings feature to name whoever made a copy.
/// A deleted account's name is forgotten.
#[async_trait]
pub trait AccountNameCatalog: Send + Sync {
    /// Keeps an account's name, unless the account was already deleted.
    async fn remember_name(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        name: &str,
    ) -> Result<(), CatalogError>;

    /// Forgets a deleted account's name, and lays a tombstone so a late sign-up does not bring it back.
    async fn forget_name(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;

    /// The names of those accounts whose name is kept.
    async fn names_of(
        &self,
        accounts: &[AccountId],
    ) -> Result<HashMap<AccountId, String>, CatalogError>;
}

/// What each reading copy holds, kept as the work stood when the copy was made.
/// It never changes, and goes once the copy is deleted.
#[async_trait]
pub trait ReadingCopyContents: Send + Sync {
    async fn keep(
        &self,
        transaction: &mut Transaction,
        copy: &ReadingCopyId,
        contents: &Contents,
    ) -> Result<(), CatalogError>;

    async fn contents_of(&self, copy: &ReadingCopyId) -> Result<Option<Contents>, CatalogError>;

    async fn forget(
        &self,
        transaction: &mut Transaction,
        copy: &ReadingCopyId,
    ) -> Result<(), CatalogError>;
}
