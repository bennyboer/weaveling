use std::sync::Arc;

use eventpublishing::published_in;
use ideas_contract::{DISCARDED, IdeaEventDTO};
use messaging::{Failure, Handler, Message};
use outline_core::{Attachment, OutlineCatalog, OutlineService};

use crate::letting_go::let_go_everywhere;

pub struct DetachOnDiscard {
    outlines: OutlineService,
    catalog: Arc<dyn OutlineCatalog>,
}

impl DetachOnDiscard {
    pub fn new(outlines: OutlineService, catalog: Arc<dyn OutlineCatalog>) -> Self {
        Self { outlines, catalog }
    }
}

#[async_trait::async_trait]
impl Handler for DetachOnDiscard {
    const NAME: &'static str = "detach-discarded-idea";
    const LISTENS_TO: &'static [&'static str] = &[DISCARDED];
    const WHEN_REFUSED: &'static str = "A discarded idea could not be taken out of the outline.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let discarded = published_in::<IdeaEventDTO>(message)?;

        let_go_everywhere(
            &self.outlines,
            self.catalog.as_ref(),
            &Attachment::idea(&discarded.aggregate.id),
        )
        .await?;

        Ok(())
    }
}
