use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;

use crate::id::AssetId;

/// One piece of what is kept for an asset: the original as it was uploaded, or a variant made from it, by its width.
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

/// Where the bytes of assets are kept, apart from their events: a folder, or the database. Nothing here is part of a transaction, as a folder takes none; a piece is kept before the asset is told uploaded, and forgotten after it is told deleted.
#[async_trait]
pub trait Contents: Send + Sync {
    /// Keeps a piece of an asset, in place of one kept before.
    async fn keep(&self, asset: &AssetId, piece: Piece, bytes: &[u8]) -> Result<(), ContentsError>;

    async fn read(&self, asset: &AssetId, piece: Piece) -> Result<Option<Vec<u8>>, ContentsError>;

    /// Forgets every piece of an asset; forgetting one never kept is no failure.
    async fn forget(&self, asset: &AssetId) -> Result<(), ContentsError>;
}

impl Piece {
    /// How the piece is named where it is kept.
    pub fn name(self) -> String {
        match self {
            Self::Original => "original".to_owned(),
            Self::Width(width) => width.to_string(),
        }
    }
}
