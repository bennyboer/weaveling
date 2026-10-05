use std::sync::Arc;

use clock::Clock;
use messaging::Message;
use passages_core::PassageChange;
use time::OffsetDateTime;

pub type PassageMessageMapping = fn(&PassageChange, OffsetDateTime) -> Message;

#[derive(Clone)]
pub(crate) struct Mapping {
    clock: Arc<dyn Clock>,
    message_for: PassageMessageMapping,
}

impl Mapping {
    pub(crate) fn new(clock: Arc<dyn Clock>, message_for: PassageMessageMapping) -> Self {
        Self { clock, message_for }
    }

    pub(crate) fn message(&self, change: &PassageChange) -> Message {
        (self.message_for)(change, self.clock.now())
    }
}
