use std::sync::Arc;

use clock::Clock;
use eventsourcing::{Agent, ServiceError};
use ideas_contract::{MORE_TO_SWEEP, MoreToSweepDTO};
use ideas_core::{IdeaCatalog, IdeaError, IdeaId, IdeaService, IdeaServiceError, ProjectLink};
use messaging::{Failure, Handler, Message, Publisher, RoutingKey};
use projects_contract::{DELETED, ProjectEventDTO};

pub const AT_MOST: usize = 100;

pub struct DiscardOnProjectDeletedListener {
    ideas: IdeaService,
    catalog: Arc<dyn IdeaCatalog>,
    publisher: Arc<dyn Publisher>,
    clock: Arc<dyn Clock>,
    at_most: usize,
}

impl DiscardOnProjectDeletedListener {
    pub fn new(
        ideas: IdeaService,
        catalog: Arc<dyn IdeaCatalog>,
        publisher: Arc<dyn Publisher>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            ideas,
            catalog,
            publisher,
            clock,
            at_most: AT_MOST,
        }
    }

    pub fn taking_at_most(mut self, at_most: usize) -> Self {
        self.at_most = at_most;
        self
    }

    async fn sweep(&self, asked: &Message) -> Result<(), Failure> {
        let (project, after) = what_to_sweep(asked)?;
        let batch = self
            .catalog
            .in_project_after(&project, after, self.at_most)
            .await?;

        let Some(last) = batch.last().map(|summary| summary.id) else {
            return Ok(());
        };

        for summary in &batch {
            match self
                .ideas
                .discard(&summary.id.to_string(), None, &nobody())
                .await
            {
                Ok(_) => {}
                Err(refused) if already_discarded(&refused) => {}
                Err(refused) => return Err(refused.into()),
            }
        }

        if batch.len() < self.at_most {
            return Ok(());
        }

        self.publisher
            .publish(self.carry_on(asked, &project, last))
            .await?;

        Ok(())
    }

    fn carry_on(&self, asked: &Message, project: &ProjectLink, after: IdeaId) -> Message {
        asked.answering(
            RoutingKey::parse(MORE_TO_SWEEP).expect("a declared routing key holds no wildcards"),
            serde_json::to_value(MoreToSweepDTO {
                project: project.to_string(),
                after: after.to_string(),
            })
            .expect("a continuation is plain data and cannot fail to serialize"),
            self.clock.now(),
        )
    }
}

fn what_to_sweep(asked: &Message) -> Result<(ProjectLink, Option<IdeaId>), Failure> {
    if asked.routing.to_string() == MORE_TO_SWEEP {
        let carried: MoreToSweepDTO = asked.payload_as()?;

        return Ok((
            ProjectLink::from(carried.project.as_str()),
            Some(carried.after.parse()?),
        ));
    }

    let deleted = eventpublishing::published_in::<ProjectEventDTO>(asked)?;

    Ok((ProjectLink::from(deleted.aggregate.id.as_str()), None))
}

fn already_discarded(refused: &IdeaServiceError) -> bool {
    matches!(
        refused,
        IdeaServiceError::Events(ServiceError::Refused(IdeaError::Discarded))
    )
}

fn nobody() -> Agent {
    Agent::System
}

#[async_trait::async_trait]
impl Handler for DiscardOnProjectDeletedListener {
    const NAME: &'static str = "discard-ideas-of-deleted-project";
    const LISTENS_TO: &'static [&'static str] = &[DELETED, MORE_TO_SWEEP];
    const WHEN_REFUSED: &'static str = "The ideas of a deleted project were not cleared away.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        self.sweep(message).await
    }
}
