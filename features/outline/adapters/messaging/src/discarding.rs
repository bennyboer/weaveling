use ideas_contract::{DISCARDED, IdeaEventDTO};
use messaging::{Failure, Handler, Message};
use outline_core::{Attachment, OutlineCatalog, OutlineService};
use std::sync::Arc;

use crate::letting_go;

pub struct DetachOnDiscardListener {
    outlines: OutlineService,
    catalog: Arc<dyn OutlineCatalog>,
}

impl DetachOnDiscardListener {
    pub fn new(outlines: OutlineService, catalog: Arc<dyn OutlineCatalog>) -> Self {
        Self { outlines, catalog }
    }
}

#[async_trait::async_trait]
impl Handler for DetachOnDiscardListener {
    const NAME: &'static str = "detach-discarded-idea";
    const LISTENS_TO: &'static [&'static str] = &[DISCARDED];
    const WHEN_REFUSED: &'static str = "A discarded idea could not be taken out of the outline.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let discarded = eventpublishing::published_in::<IdeaEventDTO>(message)?;

        letting_go::let_go_everywhere(
            &self.outlines,
            self.catalog.as_ref(),
            &Attachment::idea(&discarded.aggregate.id),
        )
        .await?;

        Ok(())
    }
}
