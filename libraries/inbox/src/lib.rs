mod memory;
#[cfg(test)]
mod suite;

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "sqlite")]
pub mod sqlite;

pub use memory::InMemoryInbox;

#[cfg(feature = "postgres")]
pub use postgres::PostgresInbox;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteInbox;

use async_trait::async_trait;
use messaging::{ListenerName, MessageId};
use thiserror::Error;
use time::OffsetDateTime;
use transacting::{Failure, Transaction};

#[derive(Debug, Error)]
pub enum InboxError {
    #[error("the inbox could not be reached: {0}")]
    Unreachable(String),
    #[error(transparent)]
    Transacting(#[from] Failure),
}

#[async_trait]
pub trait Inbox: Send + Sync {
    async fn heard_before(
        &self,
        transaction: &mut Transaction,
        listener: &ListenerName,
        message: MessageId,
    ) -> Result<bool, InboxError>;

    async fn forget_heard_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, InboxError>;
}
