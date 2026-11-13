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

/// What another feature's things belong to, as far as this feature has heard.
/// For example, an idea or a scene belongs to its project.
/// A thing has one owner of each kind.
#[async_trait]
pub trait Belongings: Send + Sync {
    /// Stores whom a thing belongs to, for one kind of owner.
    /// Does nothing if the thing was already forgotten, so a late update about a deleted thing is not kept.
    async fn remember(
        &self,
        transaction: &mut Transaction,
        kind: &str,
        id: &str,
        owner_kind: &str,
        owner: &str,
    ) -> Result<(), BelongingsError>;

    /// Forgets what a thing belongs to, for every kind of owner.
    /// It also lays a tombstone for the thing, so it is not stored again.
    async fn forget(
        &self,
        transaction: &mut Transaction,
        kind: &str,
        id: &str,
        at: OffsetDateTime,
    ) -> Result<(), BelongingsError>;

    /// Deletes at most `at_most` tombstones laid before `before`, the oldest first.
    /// Call it only once no late update can arrive any more.
    /// Returns how many were deleted.
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
