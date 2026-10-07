mod agent;
mod aggregate;
mod codec;
mod event;
mod memory;
mod metadata;
mod outbox;
mod patch;
mod publish;
mod publishing;
mod relaying;
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
pub use memory::{InMemoryEventStore, InMemoryOutbox};
pub use metadata::EventMetadata;
pub use outbox::{CLAIM_FOR, Delivered, KEPT_FOR, MessageMapping, Origin, Outbox, OutboxError};
pub use patch::{Patch, Patcher};
pub use publish::{EventPublisher, NoopEventPublisher, PublishError};
pub use publishing::PublishingEventStore;
pub use relaying::{Cadence, RelayTask};
pub use service::{Appended, EventSourcingService, ServiceError, Standing};
pub use store::{EventStore, StoreError};
pub use version::Version;

#[cfg(feature = "postgres")]
pub use postgres::{PostgresEventStore, PostgresOutbox, enqueue, migrations};

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteEventStore;
