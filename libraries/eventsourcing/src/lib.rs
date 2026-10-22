mod agent;
mod aggregate;
mod codec;
mod event;
mod memory;
mod message_mapping;
mod metadata;
mod patch;
mod service;
mod store;
mod version;

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(any(feature = "postgres", feature = "sqlite"))]
mod stored_agent;

#[cfg(test)]
mod testing;

pub use agent::{AccountId, Agent};
pub use aggregate::{Aggregate, AggregateId, AggregateType};
pub use codec::Codec;
pub use event::{Event, EventName, Recorded};
pub use memory::InMemoryEventStore;
pub use message_mapping::MessageMapping;
pub use metadata::EventMetadata;
pub use patch::{Patch, Patcher};
pub use service::{Appended, EventSourcingService, ServiceError, Standing};
pub use store::{EventStore, StoreError};
pub use version::Version;

#[cfg(feature = "postgres")]
pub use postgres::PostgresEventStore;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteEventStore;
