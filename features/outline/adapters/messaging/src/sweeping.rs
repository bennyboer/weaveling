use eventpublishing::published_in;
use eventsourcing::{Agent, ServiceError};
use messaging::{Failure, Handler, Message};
use outline_core::{OutlineError, OutlineService, OutlineServiceError, ProjectLink};
use projects_contract::{DELETED, ProjectEventDTO};

pub struct DiscardOutlinesOnProjectDeleted {
    outlines: OutlineService,
}

impl DiscardOutlinesOnProjectDeleted {
    pub fn new(outlines: OutlineService) -> Self {
        Self { outlines }
    }

    async fn discard_everything(&self, project: &ProjectLink) -> Result<(), Failure> {
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
impl Handler for DiscardOutlinesOnProjectDeleted {
    const NAME: &'static str = "discard-outlines-of-deleted-project";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str = "The outline of a deleted project was not cleared away.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let deleted = published_in::<ProjectEventDTO>(message)?;

        self.discard_everything(&ProjectLink::from(deleted.aggregate.id.as_str()))
            .await
    }
}
