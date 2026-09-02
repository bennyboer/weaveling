use std::fmt::{self, Display, Formatter};

use crate::metadata::EventMetadata;
use crate::version::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventName(&'static str);

pub trait Event {
    fn name(&self) -> EventName;

    fn version(&self) -> Version;

    fn is_snapshot(&self) -> bool {
        false
    }

    fn is_publishable(&self) -> bool {
        !self.is_snapshot()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded<E> {
    pub event: E,
    pub metadata: EventMetadata,
}

impl EventName {
    pub const fn of(named: &'static str) -> Self {
        Self(named)
    }

    pub fn as_str(self) -> &'static str {
        self.0
    }
}

impl Display for EventName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_event_name_is_nameable_at_compile_time() {
        const CAPTURED: EventName = EventName::of("PIECE_CAPTURED");

        assert_eq!(CAPTURED.as_str(), "PIECE_CAPTURED");
        assert_eq!(CAPTURED.to_string(), "PIECE_CAPTURED");
    }

    #[test]
    fn two_event_names_are_distinct() {
        assert_ne!(
            EventName::of("PIECE_CAPTURED"),
            EventName::of("PIECE_RETITLED")
        );
    }
}
