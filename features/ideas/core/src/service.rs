use std::sync::Arc;

use clock::Clock;
use eventsourcing::{Agent, AggregateId, EventSourcingService, EventStore, Standing, Version};
use transacting::{Transaction, Transactions};

use crate::error::IdeasError;
use crate::id::IdeaId;
use crate::idea::{Idea, IdeaCommand, IdeaEvent, ProjectLink};
use crate::title::IdeaTitle;

#[derive(Clone)]
pub struct IdeaService {
    events: Arc<EventSourcingService<Idea>>,
    clock: Arc<dyn Clock>,
}

impl IdeaService {
    pub fn new(
        store: Arc<dyn EventStore<IdeaEvent>>,
        transactions: Arc<dyn Transactions>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(
                store,
                transactions,
                clock.clone(),
            )),
            clock,
        }
    }

    pub async fn capture(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        title: &str,
        agent: &Agent,
    ) -> Result<IdeaId, IdeasError> {
        let id = IdeaId::generate(self.clock.now());
        let command = IdeaCommand::Capture {
            project: project.clone(),
            title: IdeaTitle::new(title)?,
        };

        self.events
            .begin(Some(transaction), &AggregateId::from(&id), command, agent)
            .await?;

        Ok(id)
    }

    pub async fn latest(&self, id: &IdeaId) -> Result<Standing<Idea>, IdeasError> {
        Ok(self.events.latest(&AggregateId::from(id)).await?)
    }

    pub async fn retitle(
        &self,
        transaction: &mut Transaction,
        id: &IdeaId,
        title: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, IdeasError> {
        self.carry_out(
            transaction,
            id,
            IdeaCommand::Retitle(IdeaTitle::new(title)?),
            expected,
            agent,
        )
        .await
    }

    pub async fn discard(
        &self,
        transaction: &mut Transaction,
        id: &IdeaId,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, IdeasError> {
        self.carry_out(transaction, id, IdeaCommand::Discard, expected, agent)
            .await
    }

    async fn carry_out(
        &self,
        transaction: &mut Transaction,
        id: &IdeaId,
        command: IdeaCommand,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, IdeasError> {
        let key = AggregateId::from(id);

        let landed = match expected {
            Some(expected) => {
                self.events
                    .execute_at(Some(transaction), &key, expected, command, agent)
                    .await?
            }
            None => {
                self.events
                    .execute(Some(transaction), &key, command, agent)
                    .await?
            }
        };

        Ok(landed.version)
    }
}
