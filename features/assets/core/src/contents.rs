use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;

use crate::id::AssetId;

/// One piece stored for an asset: the original as it was uploaded, or a variant made from it.
/// A variant is known by its width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Piece {
    Original,
    Width(u32),
}

#[derive(Debug, Error)]
pub enum ContentsError {
    #[error("the assets' contents could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

/// Where the bytes of assets are stored, apart from their events: a folder, or the database.
/// Nothing here is part of a transaction, because a folder cannot take part in one.
/// A piece is stored before the asset is recorded as uploaded.
/// It is removed after the asset is recorded as deleted.
#[async_trait]
pub trait Contents: Send + Sync {
    /// Stores a piece of an asset, replacing any piece stored before.
    async fn keep(&self, asset: &AssetId, piece: Piece, bytes: &[u8]) -> Result<(), ContentsError>;

    async fn read(&self, asset: &AssetId, piece: Piece) -> Result<Option<Vec<u8>>, ContentsError>;

    /// Removes every piece of an asset.
    /// Removing pieces that were never stored does not fail.
    async fn forget(&self, asset: &AssetId) -> Result<(), ContentsError>;

    /// Returns the ids of the assets that have pieces stored, in id order.
    /// Starts after the given id, and returns at most `at_most` ids.
    async fn stored(
        &self,
        after: Option<&AssetId>,
        at_most: u64,
    ) -> Result<Vec<AssetId>, ContentsError>;
}

impl Piece {
    /// Returns the name the piece is stored under.
    pub fn name(self) -> String {
        match self {
            Self::Original => "original".to_owned(),
            Self::Width(width) => width.to_string(),
        }
    }
}
