mod publishing;

pub use publishing::message_for;

use eventpublishing::aggregate_in;
use projects_contract::EVERY_PROJECT;
use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use eventsourcing::{AggregateId, EventSourcingService, EventStore};
use messaging::{Failure, Handler, Message};
use projects_core::{Project, ProjectCatalog, ProjectEvent, ProjectId, ProjectSummary};

pub struct CatalogueProjectsListener {
    events: EventSourcingService<Project>,
    catalog: Arc<dyn ProjectCatalog>,
}

impl CatalogueProjectsListener {
    pub fn new(
        store: Arc<dyn EventStore<ProjectEvent>>,
        catalog: Arc<dyn ProjectCatalog>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: EventSourcingService::new(store, clock),
            catalog,
        }
    }

    async fn catalogue(&self, id: &ProjectId) -> Result<(), Failure> {
        let standing = self.events.latest(&AggregateId::from(id)).await?;

        if standing.state.is_deleted() {
            self.catalog.forget(id).await?;
        } else {
            self.catalog
                .remember(&ProjectSummary::of(*id, standing.version, &standing.state))
                .await?;
        }

        Ok(())
    }
}

#[async_trait]
impl Handler for CatalogueProjectsListener {
    const NAME: &'static str = "catalogue-project";
    const LISTENS_TO: &'static [&'static str] = &[EVERY_PROJECT];
    const WHEN_REFUSED: &'static str = "A change to a project did not reach your list of projects.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        self.catalogue(&aggregate_in::<ProjectId>(message)?).await
    }
}
