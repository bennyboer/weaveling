use std::fmt::{self, Display, Formatter};

use crate::agent::Agent;
use crate::event::Event;
use crate::metadata::EventMetadata;
use crate::patch::Patch;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AggregateId(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AggregateType(&'static str);

pub trait Aggregate: Sized {
    type Command;
    type Event: Event;
    type Error: std::error::Error;

    const KIND: AggregateType;

    fn begin(command: Self::Command, agent: &Agent) -> Result<Vec<Self::Event>, Self::Error>;

    fn from_first(event: &Self::Event, metadata: &EventMetadata) -> Option<Self>;

    fn decide(
        &self,
        command: Self::Command,
        agent: &Agent,
    ) -> Result<Vec<Self::Event>, Self::Error>;

    fn apply(&mut self, event: &Self::Event, metadata: &EventMetadata);

    fn snapshot(&self) -> Self::Event;

    fn snapshot_after(&self) -> Option<u32> {
        Some(100)
    }

    fn patches() -> Vec<Patch<Self::Event>> {
        Vec::new()
    }
}

impl AggregateId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for AggregateId {
    fn from(given: String) -> Self {
        Self(given)
    }
}

impl From<&str> for AggregateId {
    fn from(given: &str) -> Self {
        Self(given.to_owned())
    }
}

impl Display for AggregateId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl AggregateType {
    pub const fn of(named: &'static str) -> Self {
        Self(named)
    }

    pub fn as_str(self) -> &'static str {
        self.0
    }
}

impl Display for AggregateType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: AggregateType = AggregateType::of("sample");

    #[test]
    fn an_aggregate_type_is_nameable_at_compile_time() {
        assert_eq!(SAMPLE.as_str(), "sample");
        assert_eq!(SAMPLE.to_string(), "sample");
    }

    #[test]
    fn two_kinds_of_aggregate_are_not_the_same() {
        assert_ne!(SAMPLE, AggregateType::of("board"));
    }

    #[test]
    fn an_aggregate_id_keeps_whatever_the_feature_gave_it() {
        let id = AggregateId::from("piece_019a4f2b");

        assert_eq!(id.as_str(), "piece_019a4f2b");
        assert_eq!(id.to_string(), "piece_019a4f2b");
    }
}
