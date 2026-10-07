mod memory;
mod outbox;
mod relaying;

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "sqlite")]
pub mod sqlite;

pub use memory::InMemoryOutbox;
pub use outbox::{CLAIM_FOR, Delivered, KEPT_FOR, Origin, Outbox, OutboxError};
pub use relaying::{Cadence, RelayTask};

#[cfg(feature = "postgres")]
pub use postgres::PostgresOutbox;
