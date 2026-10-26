use ideas_contract::{DISCARDED, IdeaEventDTO};
use messaging::{Failure, Handler, Message};
use outline_core::{Attachment, OutlinesCore};

pub struct DetachOnDiscardListener {
    outlines: OutlinesCore,
}

impl DetachOnDiscardListener {
    pub fn new(outlines: OutlinesCore) -> Self {
        Self { outlines }
    }
}

#[async_trait::async_trait]
impl Handler for DetachOnDiscardListener {
    const NAME: &'static str = "detach-discarded-idea";
    const LISTENS_TO: &'static [&'static str] = &[DISCARDED];
    const WHEN_REFUSED: &'static str = "A discarded idea could not be taken out of the outline.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let discarded = eventpublishing::published_in::<IdeaEventDTO>(message)?;

        Ok(self
            .outlines
            .let_go_everywhere(
                &Attachment::idea(&discarded.aggregate.id),
                message.occurred_at,
            )
            .await?)
    }
}
