use time::OffsetDateTime;

use crate::agent::Agent;
use crate::aggregate::{AggregateId, AggregateType};
use crate::version::Version;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventMetadata {
    pub aggregate: AggregateId,
    pub kind: AggregateType,
    pub version: Version,
    pub agent: Agent,
    pub occurred_at: OffsetDateTime,
    pub is_snapshot: bool,
}
