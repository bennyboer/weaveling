use messaging::{Failure, Handler, Message};
use outline_contract::{ATTACHED, DETACHED, SECTION_REMOVED};
use outline_core::{OutlineId, OutlinesCore};

pub struct IndexAttachmentsListener {
    outlines: OutlinesCore,
}

impl IndexAttachmentsListener {
    pub fn new(outlines: OutlinesCore) -> Self {
        Self { outlines }
    }
}

#[async_trait::async_trait]
impl Handler for IndexAttachmentsListener {
    const NAME: &'static str = "index-attachments";
    const LISTENS_TO: &'static [&'static str] = &[ATTACHED, DETACHED, SECTION_REMOVED];
    const WHEN_REFUSED: &'static str =
        "A change to the outline did not reach the record of what it holds.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        Ok(self
            .outlines
            .index_attachments(&eventpublishing::aggregate_in::<OutlineId>(message)?)
            .await?)
    }
}
