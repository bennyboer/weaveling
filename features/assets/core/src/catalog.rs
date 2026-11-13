use std::collections::HashSet;
use std::error::Error;

use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use thiserror::Error;
use time::OffsetDateTime;
use transacting::Transaction;

use crate::asset::{Format, Owner, ProjectLink};
use crate::id::AssetId;
use crate::limits::Usage;

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("the list of assets could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

/// An asset as the catalog stores it.
/// It holds its owner, its uploader, the picture's format and size, and all bytes stored for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetSummary {
    pub id: AssetId,
    pub owner: Owner,
    pub uploader: Option<AccountId>,
    pub format: Format,
    pub width: u32,
    pub height: u32,
    pub original_size: u64,
    pub total_size: u64,
    pub uploaded_at: OffsetDateTime,
}

/// Whose storage use is listed: the accounts' or the projects'.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holders {
    Accounts,
    Projects,
}

/// The order of the storage overview: the largest use first, or by id.
/// Entries with the same use are ordered by id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sorted {
    MostUsed,
    ById,
}

/// How many bytes one account or project uses, by its id.
/// Also holds its name, and for an account its email.
/// Both are missing if the assets feature never heard them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub id: String,
    pub total: u64,
    pub name: Option<String>,
    pub email: Option<String>,
}

/// The order of an account's uploads: the largest first, or the newest first.
/// Uploads with the same size or time are ordered by id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadsSorted {
    Largest,
    Newest,
}

/// One of an account's uploads, as listed.
/// It holds the owner, the image's format and size, how many bytes it uses, and when it was uploaded.
/// For an upload into a project it also holds the project's name, if the assets feature heard it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upload {
    pub id: AssetId,
    pub owner: Owner,
    pub project_name: Option<String>,
    pub format: Format,
    pub width: u32,
    pub height: u32,
    pub total_size: u64,
    pub uploaded_at: OffsetDateTime,
}

/// The cursor for the next page of uploads: the last listed upload's size, time and id.
/// It holds both size and time, so it works for either order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadMark {
    pub id: AssetId,
    pub total_size: u64,
    pub uploaded_at: OffsetDateTime,
}

impl From<&Upload> for UploadMark {
    fn from(upload: &Upload) -> Self {
        Self {
            id: upload.id,
            total_size: upload.total_size,
            uploaded_at: upload.uploaded_at,
        }
    }
}

/// An account's name and email, as reported by the accounts feature.
/// Operators see them in the storage overview and can search by them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub account: AccountId,
    pub name: String,
    pub email: String,
}

/// The stored assets.
/// They are written in the same transaction that records an upload or a deletion.
/// So each bound's usage is never out of date when an upload is checked against it.
#[async_trait]
pub trait AssetCatalog: Send + Sync {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &AssetSummary,
    ) -> Result<(), CatalogError>;

    /// Forgets an asset.
    /// Forgetting one that was never stored does not fail.
    async fn forget(&self, transaction: &mut Transaction, id: &AssetId)
    -> Result<(), CatalogError>;

    /// Returns how many bytes the installation stores, the account uploaded, and the project stores.
    /// The account's and the project's usage are only read if they are given.
    async fn usage_within(
        &self,
        transaction: &mut Transaction,
        uploader: Option<&AccountId>,
        project: Option<&ProjectLink>,
    ) -> Result<Usage, CatalogError>;

    /// Lists the storage use of accounts or projects, starting after the given cursor.
    /// Returns at most `at_most` entries, and only those that use something.
    /// With a term, only entries whose name (or an account's email) contains it, ignoring case.
    async fn held_by(
        &self,
        transaction: &mut Transaction,
        holders: Holders,
        sorted: Sorted,
        after: Option<&Held>,
        matching: Option<&str>,
        at_most: u64,
    ) -> Result<Vec<Held>, CatalogError>;

    /// Returns which of the given assets the catalog holds.
    /// An asset is held from its upload until it is deleted.
    async fn held_of(
        &self,
        transaction: &mut Transaction,
        ids: &[AssetId],
    ) -> Result<HashSet<AssetId>, CatalogError>;

    /// Returns the ids of an owner's assets, ordered by id, at most `at_most` of them.
    async fn owned_by(
        &self,
        transaction: &mut Transaction,
        owner: &Owner,
        at_most: u64,
    ) -> Result<Vec<AssetId>, CatalogError>;

    /// Lists what the account uploaded, both into projects and as its own images.
    /// Starts after the given cursor and returns at most `at_most` uploads.
    async fn uploaded_by(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        sorted: UploadsSorted,
        after: Option<&UploadMark>,
        at_most: u64,
    ) -> Result<Vec<Upload>, CatalogError>;

    /// Stores an account's name and email at a version.
    /// It is ignored if a newer version is already stored.
    async fn remember_person(
        &self,
        transaction: &mut Transaction,
        person: &Person,
        version: Version,
    ) -> Result<(), CatalogError>;

    /// Removes an account's name and email at a version, for example when the account is deleted.
    /// A tombstone stays behind, so an older message arriving late cannot bring the name back.
    async fn forget_person(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    /// Stores a project's name at a version.
    /// It is ignored if a newer version is already stored.
    async fn remember_project_name(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        version: Version,
        name: &str,
    ) -> Result<(), CatalogError>;

    /// Removes a project's name at a version, for example when the project is deleted.
    /// A tombstone stays behind, so an older message arriving late cannot bring the name back.
    async fn forget_project_name(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    /// Deletes the name tombstones laid before the given time, at most `at_most` of each kind.
    /// Returns how many were deleted.
    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;
}
