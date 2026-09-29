mod agent;
mod aggregate;
mod codec;
mod event;
mod memory;
mod metadata;
mod outbox;
mod patch;
#[cfg(feature = "postgres")]
mod postgres;
mod publish;
mod publishing;
mod relaying;
mod service;
mod store;
mod version;

pub mod testing;

pub use agent::{Agent, AgentId};
pub use aggregate::{Aggregate, AggregateId, AggregateType};
pub use codec::Codec;
pub use event::{Event, EventName, Recorded};
pub use memory::{InMemoryEventStore, InMemoryOutbox};
pub use metadata::EventMetadata;
pub use outbox::{CLAIM_FOR, Delivered, KEPT_FOR, MessageMapping, Outbox, OutboxError};
pub use patch::{Patch, Patcher};
#[cfg(feature = "postgres")]
pub use postgres::{PostgresEventStore, PostgresOutbox, migrations};
pub use publish::{EventPublisher, NoopEventPublisher, PublishError};
pub use publishing::PublishingEventStore;
pub use relaying::{Cadence, RelayTask};
pub use service::{Appended, EventSourcingService, ServiceError, Standing};
pub use store::{EventStore, StoreError};
pub use version::Version;
