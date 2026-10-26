use messaging::{Failure, Handler, Message};
use outline_core::OutlinesCore;
use projects_contract::{DELETED, ProjectEventDTO};

pub struct DiscardOutlinesOnProjectDeletedListener {
    outlines: OutlinesCore,
}

impl DiscardOutlinesOnProjectDeletedListener {
    pub fn new(outlines: OutlinesCore) -> Self {
        Self { outlines }
    }
}

#[async_trait::async_trait]
impl Handler for DiscardOutlinesOnProjectDeletedListener {
    const NAME: &'static str = "discard-outlines-of-deleted-project";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str = "The outline of a deleted project was not cleared away.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let deleted = eventpublishing::published_in::<ProjectEventDTO>(message)?;

        Ok(self.outlines.sweep(&deleted.aggregate.id).await?)
    }
}
