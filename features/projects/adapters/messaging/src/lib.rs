mod publishing;

pub use publishing::{
    ProjectEventPublisher, UnreadableProjectEvent, event_in, every_event, message_for, project_in,
};

use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use eventsourcing::{AggregateId, EventSourcingService, EventStore, ServiceError};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use projects_core::{
    CatalogError, Project, ProjectCatalog, ProjectError, ProjectEvent, ProjectId, ProjectSummary,
};
use thiserror::Error;

const NAME: &str = "catalogue-project";

pub struct ProjectCatalogProjector {
    events: EventSourcingService<Project>,
    catalog: Arc<dyn ProjectCatalog>,
}

#[derive(Debug, Error)]
enum NotCatalogued {
    #[error(transparent)]
    Unreadable(#[from] UnreadableProjectEvent),
    #[error(transparent)]
    Events(#[from] ServiceError<ProjectError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

impl ProjectCatalogProjector {
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

    async fn catalogue(&self, id: &ProjectId) -> Result<(), NotCatalogued> {
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

    async fn handle(&self, message: &Message) -> Result<(), NotCatalogued> {
        self.catalogue(&project_in(message)?).await
    }
}

#[async_trait]
impl Listener for ProjectCatalogProjector {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the catalog listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![every_event()]
    }

    fn when_refused(&self) -> &'static str {
        "A change to a project did not reach your list of projects."
    }

    fn delivery(&self) -> Delivery {
        Delivery::Kept
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        self.handle(message)
            .await
            .map_err(|why| NotHandled::because(self.named(), message.routing.clone(), why))
    }
}
