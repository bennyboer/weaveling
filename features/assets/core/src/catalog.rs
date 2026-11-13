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

/// An asset as the list holds it: whose it is, who uploaded it, the picture's kind and size, and every byte kept for it.
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

/// Whose holdings are listed: every account's that uploaded, or every project's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holders {
    Accounts,
    Projects,
}

/// How holdings are listed: the most held first, the ids in order where they hold as much; or by id alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sorted {
    MostUsed,
    ById,
}

/// What one account or project holds, in bytes, by its id, and its name and an account's address as far as the assets heard, none where they did not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub id: String,
    pub total: u64,
    pub name: Option<String>,
    pub email: Option<String>,
}

/// How an account's uploads are listed: the largest first, or the newest; the ids in order where they are alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadsSorted {
    Largest,
    Newest,
}

/// One of an account's uploads as listed: whose it is, the project's name as far as the assets heard it, the picture, what it takes, and when it was uploaded.
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

/// The upload a page goes on after, by what it takes, when it was uploaded and its id, whichever way the page is sorted.
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

/// An account as the assets heard of it from the accounts: whom to name in the operators' administration, and to find there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub account: AccountId,
    pub name: String,
    pub email: String,
}

/// The assets kept, written in the transaction that tells one uploaded or deleted, so what each bound holds is never behind when an upload is weighed against it.
#[async_trait]
pub trait AssetCatalog: Send + Sync {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &AssetSummary,
    ) -> Result<(), CatalogError>;

    /// Forgets an asset; forgetting one never remembered is no failure.
    async fn forget(&self, transaction: &mut Transaction, id: &AssetId)
    -> Result<(), CatalogError>;

    /// What the installation holds, what the account uploaded, and what the project holds, each where named.
    async fn usage_within(
        &self,
        transaction: &mut Transaction,
        uploader: Option<&AccountId>,
        project: Option<&ProjectLink>,
    ) -> Result<Usage, CatalogError>;

    /// What the accounts or the projects holding anything hold, at most so many, after the one listed last as sorted; where a term is given, those alone whose name, or an account's address, holds it, whatever its case.
    async fn held_by(
        &self,
        transaction: &mut Transaction,
        holders: Holders,
        sorted: Sorted,
        after: Option<&Held>,
        matching: Option<&str>,
        at_most: u64,
    ) -> Result<Vec<Held>, CatalogError>;

    /// What the account uploaded, into projects and its own, at most so many, after the one listed last as sorted.
    async fn uploaded_by(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        sorted: UploadsSorted,
        after: Option<&UploadMark>,
        at_most: u64,
    ) -> Result<Vec<Upload>, CatalogError>;

    /// Remembers an account's name and address as of a version of it, the newest heard winning.
    async fn remember_person(
        &self,
        transaction: &mut Transaction,
        person: &Person,
        version: Version,
    ) -> Result<(), CatalogError>;

    /// Forgets an account's name and address as of a version, as when it is deleted, keeping what guards against an older word heard late.
    async fn forget_person(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    /// Remembers a project's name as of a version of it, the newest heard winning.
    async fn remember_project_name(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        version: Version,
        name: &str,
    ) -> Result<(), CatalogError>;

    /// Forgets a project's name as of a version, as when it is deleted, keeping what guards against an older word heard late.
    async fn forget_project_name(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    /// Lets go, at most so many of each, of the names forgotten before then, once nothing about them can be heard late any more: how many.
    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;
}
