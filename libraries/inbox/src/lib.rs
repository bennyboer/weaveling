mod forgetting;
mod kept;
#[cfg(test)]
mod kept_tests;
mod memory;
#[cfg(test)]
mod suite;

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "sqlite")]
pub mod sqlite;

pub use forgetting::HeardMessages;
pub use kept::{EffectHandler, Inboxed, InboxedEffect, KeptHandler};
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
    /// Notes that `listener` heard `message`, and answers whether it had already: `true` means
    /// heard before, so leave it; `false` means this is the first hearing, now noted.
    ///
    /// Asking and noting are one write, so two deliveries of one message cannot both be told
    /// `false`. The note is part of `transaction`: rolled back with the handler's own writes, the
    /// message counts as never heard and is taken in anew when it comes again. A kept listener
    /// therefore calls this in the transaction its handler writes in (see `Inboxed`).
    async fn heard_before(
        &self,
        transaction: &mut Transaction,
        listener: &ListenerName,
        message: MessageId,
    ) -> Result<bool, InboxError>;

    /// Answers whether `listener` heard `message` already, noting nothing.
    ///
    /// For work that cannot be rolled back with the inbox, such as a mail (see `InboxedEffect`):
    /// ask with this, do the work holding no transaction, then note the message with
    /// `heard_before` in a short transaction of its own. Noting first would lose the work if it
    /// failed, the message already counting as heard; the price of noting after is that a crash
    /// between the work and the note, or two deliveries at the same moment, does the work twice.
    async fn was_heard(
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
