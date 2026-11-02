mod memory;
mod purging;
#[cfg(test)]
mod suite;

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "sqlite")]
pub mod sqlite;

pub use memory::InMemoryTrash;
pub use purging::{PURGED_AT_MOST, Purge, PurgeTheTrash};

#[cfg(feature = "postgres")]
pub use postgres::PostgresTrash;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteTrash;

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;
use transacting::{Failure, Transaction};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Trashed {
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Error)]
pub enum TrashError {
    #[error("the trash could not be reached: {0}")]
    Unreachable(String),
    #[error(transparent)]
    Transacting(#[from] Failure),
}

#[async_trait]
pub trait Trash: Send + Sync {
    async fn put_into(
        &self,
        transaction: &mut Transaction,
        kind: &str,
        id: &str,
        until: OffsetDateTime,
    ) -> Result<(), TrashError>;

    async fn take_out(
        &self,
        transaction: &mut Transaction,
        kind: &str,
        id: &str,
    ) -> Result<bool, TrashError>;

    async fn due(&self, at: OffsetDateTime, at_most: u64) -> Result<Vec<Trashed>, TrashError>;
}
