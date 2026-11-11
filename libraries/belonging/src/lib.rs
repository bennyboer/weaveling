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
use transacting::Transaction;

#[derive(Debug, Error)]
pub enum BelongingsError {
    #[error("what belongs where could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

/// What another feature's things belong to, as far as this feature has heard: an idea or a scene to its project, one owner of each kind.
#[async_trait]
pub trait Belongings: Send + Sync {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        kind: &str,
        id: &str,
        owner_kind: &str,
        owner: &str,
    ) -> Result<(), BelongingsError>;

    /// Forgets what a thing belongs to, its owners of every kind.
    async fn forget(
        &self,
        transaction: &mut Transaction,
        kind: &str,
        id: &str,
    ) -> Result<(), BelongingsError>;

    async fn owner_of(
        &self,
        kind: &str,
        id: &str,
        owner_kind: &str,
    ) -> Result<Option<String>, BelongingsError>;
}
