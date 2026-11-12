use std::error::Error;

use async_trait::async_trait;
use eventsourcing::AccountId;
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
}
