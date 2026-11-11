mod memory;
#[cfg(test)]
mod suite;

#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests;

pub use memory::InMemoryBelongings;

#[cfg(feature = "postgres")]
pub use postgres::PostgresBelongings;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteBelongings;

use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;
use transacting::Transaction;

#[derive(Debug, Error)]
pub enum BelongingsError {
    #[error("what belongs where could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

/// What another feature's things belong to, as far as this feature has heard: an idea or a scene to its project, one owner of each kind.
#[async_trait]
pub trait Belongings: Send + Sync {
    /// Remembers whom a thing belongs to, of a kind of owner, unless the thing was forgotten: what is heard of it late, once it is gone, is not kept.
    async fn remember(
        &self,
        transaction: &mut Transaction,
        kind: &str,
        id: &str,
        owner_kind: &str,
        owner: &str,
    ) -> Result<(), BelongingsError>;

    /// Forgets what a thing belongs to, its owners of every kind, and lays a tombstone for it, so it is not remembered again.
    async fn forget(
        &self,
        transaction: &mut Transaction,
        kind: &str,
        id: &str,
        at: OffsetDateTime,
    ) -> Result<(), BelongingsError>;

    /// Forgets at most so many tombstones laid before then, the oldest first, once nothing heard late can come any more: how many.
    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, BelongingsError>;

    async fn owner_of(
        &self,
        kind: &str,
        id: &str,
        owner_kind: &str,
    ) -> Result<Option<String>, BelongingsError>;
}
