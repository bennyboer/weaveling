mod agent;
mod aggregate;
mod event;
mod memory;
mod metadata;
mod patch;
mod publish;
mod service;
mod store;
mod version;

pub mod testing;

pub use agent::{Agent, AgentId};
pub use aggregate::{Aggregate, AggregateId, AggregateType};
pub use event::{Event, EventName, Recorded};
pub use memory::InMemoryEventStore;
pub use metadata::EventMetadata;
pub use patch::{Patch, Patcher};
pub use publish::{EventPublisher, NoopEventPublisher, PublishError};
pub use service::{Appended, EventSourcingService, ServiceError, Standing};
pub use store::{EventStore, StoreError};
pub use version::Version;
