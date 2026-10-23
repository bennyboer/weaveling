use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use eventsourcing::{AggregateId, EventSourcingService, EventStore};
use invitations_contract::EVERY_INVITATION;
use invitations_core::{
    Invitation, InvitationCatalog, InvitationEvent, InvitationId, InvitationSummary,
};
use messaging::{Failure, Handler, Message};

pub struct CatalogueInvitationsListener {
    events: EventSourcingService<Invitation>,
    catalog: Arc<dyn InvitationCatalog>,
}

impl CatalogueInvitationsListener {
    pub fn new(
        store: Arc<dyn EventStore<InvitationEvent>>,
        catalog: Arc<dyn InvitationCatalog>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: EventSourcingService::new(store, clock),
            catalog,
        }
    }
}

#[async_trait]
impl Handler for CatalogueInvitationsListener {
    const NAME: &'static str = "catalogue-invitation";
    const LISTENS_TO: &'static [&'static str] = &[EVERY_INVITATION];
    const WHEN_REFUSED: &'static str =
        "A change to an invitation did not reach the operators' list.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let id = eventpublishing::aggregate_in::<InvitationId>(message)?;
        let standing = self.events.latest(&AggregateId::from(&id)).await?;

        Ok(self
            .catalog
            .remember(&InvitationSummary::of(
                id,
                standing.version,
                &standing.state,
            ))
            .await?)
    }
}
