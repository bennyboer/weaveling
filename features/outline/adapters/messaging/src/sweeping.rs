use eventpublishing::{UnreadableMessage, published_in};
use eventsourcing::{Agent, ServiceError};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use outline_core::{OutlineError, OutlineService, OutlineServiceError, ProjectLink};
use projects_contract::{DELETED, ProjectEventDTO};
use thiserror::Error;

const NAME: &str = "discard-outlines-of-deleted-project";

pub struct DiscardOutlinesOnProjectDeleted {
    outlines: OutlineService,
}

#[derive(Debug, Error)]
enum NotSwept {
    #[error(transparent)]
    Unreadable(#[from] UnreadableMessage),
    #[error(transparent)]
    Refused(#[from] OutlineServiceError),
}

pub fn when_project_deleted() -> Subscription {
    Subscription::parse(DELETED).expect("a declared routing key holds no wildcards")
}

impl DiscardOutlinesOnProjectDeleted {
    pub fn new(outlines: OutlineService) -> Self {
        Self { outlines }
    }

    async fn discard_everything(&self, project: &ProjectLink) -> Result<(), NotSwept> {
        let Some(outline) = self.outlines.outline_of(project.as_str()).await? else {
            return Ok(());
        };

        match self
            .outlines
            .discard(&outline.to_string(), None, &nobody())
            .await
        {
            Ok(_) => Ok(()),
            Err(refused) if already_discarded(&refused) => Ok(()),
            Err(refused) => Err(refused.into()),
        }
    }

    async fn work_through(&self, message: &Message) -> Result<(), NotSwept> {
        let deleted = published_in::<ProjectEventDTO>(message)?;

        self.discard_everything(&ProjectLink::from(deleted.aggregate.id.as_str()))
            .await
    }
}

fn already_discarded(refused: &OutlineServiceError) -> bool {
    matches!(
        refused,
        OutlineServiceError::Events(ServiceError::Refused(OutlineError::Discarded))
    )
}

fn nobody() -> Agent {
    Agent::System
}

#[async_trait::async_trait]
impl Listener for DiscardOutlinesOnProjectDeleted {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the sweep listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![when_project_deleted()]
    }

    fn delivery(&self) -> Delivery {
        Delivery::Kept
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        self.work_through(message)
            .await
            .map_err(|why| NotHandled::because(self.named(), message.routing.clone(), why))
    }
}
