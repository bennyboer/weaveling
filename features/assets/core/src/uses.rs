use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use time::OffsetDateTime;
use transacting::Transaction;

use crate::catalog::CatalogError;
use crate::id::AssetId;

/// What an asset is used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Purpose {
    /// As an account's picture. The holder is that account.
    AccountPicture,
}

/// What uses an asset.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Holder {
    Account(AccountId),
}

/// One use of an asset: what it is used for, and by what.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetUse {
    pub purpose: Purpose,
    pub by: Holder,
}

/// What uses which asset, as the features using assets reported it.
///
/// Each holder uses at most one asset for each purpose, for example one picture per account.
/// Each report comes at a version of the holder, and an older report arriving late changes nothing.
/// When a holder stops using an asset, a tombstone stays behind, so a late, older report cannot bring the use back.
#[async_trait]
pub trait AssetUses: Send + Sync {
    /// Stores that the asset is used for this purpose by the holder, replacing what the holder used for it before.
    /// It is ignored if a newer version is already stored for the holder and purpose.
    async fn use_as(
        &self,
        transaction: &mut Transaction,
        asset: &AssetId,
        purpose: Purpose,
        by: &Holder,
        version: Version,
    ) -> Result<(), CatalogError>;

    /// Stores that the holder no longer uses any asset for this purpose.
    /// A tombstone laid at `at` stays behind. It is ignored if a newer version is already stored.
    async fn stop_using(
        &self,
        transaction: &mut Transaction,
        purpose: Purpose,
        by: &Holder,
        version: Version,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    /// Returns the uses of the given assets, each with the asset it uses, in no particular order.
    /// An asset nothing uses is left out.
    async fn uses_of(
        &self,
        transaction: &mut Transaction,
        assets: &[AssetId],
    ) -> Result<Vec<(AssetId, AssetUse)>, CatalogError>;

    /// Deletes the tombstones laid before the given time, at most `at_most` of them.
    /// Returns how many were deleted.
    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;
}
