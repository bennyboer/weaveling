use std::sync::Arc;

use clock::Clock;
use messaging::Message;
use scenes_core::SceneChange;
use time::OffsetDateTime;

pub type SceneMessageMapping = fn(&SceneChange, OffsetDateTime) -> Message;

#[derive(Clone)]
pub(crate) struct Mapping {
    clock: Arc<dyn Clock>,
    message_for: SceneMessageMapping,
}

impl Mapping {
    pub(crate) fn new(clock: Arc<dyn Clock>, message_for: SceneMessageMapping) -> Self {
        Self { clock, message_for }
    }

    pub(crate) fn message(&self, change: &SceneChange) -> Message {
        (self.message_for)(change, self.clock.now())
    }
}
