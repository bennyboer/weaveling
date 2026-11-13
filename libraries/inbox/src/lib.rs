mod forgetting;
mod hearing;
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
pub use hearing::Hearing;
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
use transacting::{Failure, LostRace, Transaction};

#[derive(Debug, Error)]
pub enum InboxError {
    #[error("the inbox could not be reached: {0}")]
    Unreachable(String),
    #[error(transparent)]
    Transacting(#[from] Failure),
}

/// A failure in a transaction run by `retrying!`.
/// It is retried only when the transaction lost a race, which the transaction itself knows.
pub(crate) struct Failed(pub(crate) Box<dyn std::error::Error + Send + Sync>);

impl LostRace for Failed {
    fn lost_race(&self) -> bool {
        false
    }
}

impl<E: Into<Box<dyn std::error::Error + Send + Sync>>> From<E> for Failed {
    fn from(failure: E) -> Self {
        Failed(failure.into())
    }
}

#[async_trait]
pub trait Inbox: Send + Sync {
    /// Notes that `listener` received `message`, and returns whether it had received it before.
    /// `true` means it was received before, so skip it.
    /// `false` means this is the first time, and it is now noted.
    ///
    /// Checking and noting happen in one write, so two deliveries of one message cannot both get `false`.
    /// The note is part of `transaction`.
    /// If the transaction is rolled back with the handler's own writes, the message counts as never received.
    /// It is then handled again when it arrives again.
    /// So a kept listener calls this in the transaction its handler writes in (see `Inboxed`).
    async fn heard_before(
        &self,
        transaction: &mut Transaction,
        listener: &ListenerName,
        message: MessageId,
    ) -> Result<bool, InboxError>;

    /// Returns whether `listener` already received `message`, without noting anything.
    ///
    /// This is for work that cannot be rolled back with the inbox, such as sending a mail (see `InboxedEffect`).
    /// First check with this, then do the work without holding a transaction.
    /// Then note the message with `heard_before`, in a short transaction of its own.
    /// Noting first would lose the work if it failed, because the message would already count as received.
    /// The cost of noting afterwards: a crash between the work and the note does the work twice.
    /// So do two deliveries that arrive at the same moment.
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
