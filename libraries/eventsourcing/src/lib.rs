mod agent;
mod aggregate;
mod codec;
mod event;
mod memory;
mod message_mapping;
mod metadata;
mod patch;
mod publish;
mod publishing;
mod service;
mod store;
mod version;

#[cfg(feature = "postgres")]
mod postgres;

#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(any(feature = "postgres", feature = "sqlite"))]
mod stored_agent;

pub mod testing;

pub use agent::{Agent, AgentId};
pub use aggregate::{Aggregate, AggregateId, AggregateType};
pub use codec::Codec;
pub use event::{Event, EventName, Recorded};
pub use memory::InMemoryEventStore;
pub use message_mapping::MessageMapping;
pub use metadata::EventMetadata;
pub use patch::{Patch, Patcher};
pub use publish::{EventPublisher, NoopEventPublisher, PublishError};
pub use publishing::PublishingEventStore;
pub use service::{Appended, EventSourcingService, ServiceError, Standing};
pub use store::{EventStore, StoreError};
pub use version::Version;

#[cfg(feature = "postgres")]
pub use postgres::{PostgresEventStore, migrations};

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteEventStore;
