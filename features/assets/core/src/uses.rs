use std::collections::BTreeSet;
use std::fmt::{self, Display, Formatter};

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
    /// As an image in a scene. The holder is that scene.
    SceneImage,
}

/// A scene, as the scenes feature names it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SceneLink(String);

impl SceneLink {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SceneLink {
    fn from(given: &str) -> Self {
        Self(given.to_owned())
    }
}

impl Display for SceneLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

/// What uses an asset.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Holder {
    Account(AccountId),
    Scene(SceneLink),
}

/// One use of an asset: what it is used for, and by what.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetUse {
    pub purpose: Purpose,
    pub by: Holder,
}

/// The version after which nothing can change what a holder uses.
/// It is used when a holder is gone for good, so no late report can bring its uses back.
/// It is the highest version every backend can store.
pub const LAST_VERSION: Version = Version::of(i64::MAX as u64);

/// What uses which asset, as the features using assets reported it.
///
/// A holder uses a set of assets for each purpose, for example one picture per account, or the images in a scene.
/// Each report tells the whole set, at a version of the holder. An older report arriving late changes nothing.
/// When a holder stops using any asset for a purpose, a tombstone stays behind, so a late, older report cannot bring the uses back.
#[async_trait]
pub trait AssetUses: Send + Sync {
    /// Stores that the holder uses exactly these assets for this purpose, replacing what it used for it before.
    /// It is ignored unless the version is newer than the one stored for the holder and purpose.
    /// An empty set lays a tombstone at `at`.
    async fn used_for(
        &self,
        transaction: &mut Transaction,
        purpose: Purpose,
        by: &Holder,
        assets: &BTreeSet<AssetId>,
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
    /// A tombstone is a holder that uses nothing for a purpose any more.
    /// Returns how many were deleted.
    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;
}
