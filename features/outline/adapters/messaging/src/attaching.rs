use eventpublishing::aggregate_in;
use outline_contract::{ATTACHED, DETACHED, SECTION_REMOVED};
use std::sync::Arc;

use clock::Clock;
use eventsourcing::{AggregateId, EventSourcingService, EventStore};
use messaging::{Failure, Handler, Message};
use outline_core::{Outline, OutlineCatalog, OutlineEvent, OutlineId, OutlineService};

use crate::letting_go::let_go_of_what_is_gone;

pub struct IndexAttachmentsListener {
    events: EventSourcingService<Outline>,
    catalog: Arc<dyn OutlineCatalog>,
    outlines: OutlineService,
}

impl IndexAttachmentsListener {
    pub fn new(
        store: Arc<dyn EventStore<OutlineEvent>>,
        catalog: Arc<dyn OutlineCatalog>,
        outlines: OutlineService,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: EventSourcingService::new(store, clock),
            catalog,
            outlines,
        }
    }

    async fn index(&self, outline: &OutlineId) -> Result<(), Failure> {
        let standing = self.events.latest(&AggregateId::from(outline)).await?;
        if standing.state.is_discarded() {
            self.catalog.holds(*outline, &[]).await?;
            return Ok(());
        }

        self.catalog
            .holds(*outline, &standing.state.attachments())
            .await?;
        let_go_of_what_is_gone(&self.outlines, self.catalog.as_ref(), *outline).await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl Handler for IndexAttachmentsListener {
    const NAME: &'static str = "index-attachments";
    const LISTENS_TO: &'static [&'static str] = &[ATTACHED, DETACHED, SECTION_REMOVED];
    const WHEN_REFUSED: &'static str =
        "A change to the outline did not reach the record of what it holds.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        self.index(&aggregate_in::<OutlineId>(message)?).await
    }
}
