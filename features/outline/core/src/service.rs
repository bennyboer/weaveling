use std::sync::Arc;

use clock::Clock;
use eventsourcing::{
    Agent, AggregateId, Appended, EventSourcingService, EventStore, ServiceError, Standing, Version,
};
use registry::Registry;
use transacting::Transaction;

use crate::error::OutlinesError;
use crate::id::{OutlineId, SectionId};
use crate::outline::{KIND, Outline, OutlineCommand, OutlineEvent, ProjectLink};

#[derive(Clone)]
pub struct OutlineService {
    events: Arc<EventSourcingService<Outline>>,
    registry: Arc<dyn Registry>,
    clock: Arc<dyn Clock>,
}

pub struct Open {
    pub id: OutlineId,
    pub standing: Standing<Outline>,
}

pub struct Added {
    pub section: SectionId,
    pub version: Version,
    pub standing: Standing<Outline>,
}

pub struct Changed {
    pub version: Version,
    pub attachments_changed_in: Vec<SectionId>,
    pub standing: Standing<Outline>,
}

pub fn attachments_changed_in(landed: &Appended<OutlineEvent>) -> Vec<SectionId> {
    let mut changed_in = Vec::new();

    for happened in &landed.events {
        let section = match &happened.event {
            OutlineEvent::Attached { to, .. } => to,
            OutlineEvent::Detached { from, .. } => from,
            _ => continue,
        };
        if !changed_in.contains(section) {
            changed_in.push(*section);
        }
    }

    changed_in
}

impl OutlineService {
    pub fn new(
        store: Arc<dyn EventStore<OutlineEvent>>,
        registry: Arc<dyn Registry>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(store, clock.clone())),
            registry,
            clock,
        }
    }

    pub async fn claim(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        agent: &Agent,
    ) -> Result<OutlineId, OutlinesError> {
        let mine = OutlineId::generate(self.clock.now());
        let held = self
            .registry
            .claim(
                transaction,
                KIND.as_str(),
                project.as_str(),
                &mine.to_string(),
            )
            .await?;
        let id: OutlineId = held.parse()?;
        if id == mine {
            self.events
                .begin(
                    transaction,
                    &AggregateId::from(&id),
                    OutlineCommand::Start {
                        project: project.clone(),
                    },
                    agent,
                )
                .await?;
        }

        Ok(id)
    }

    pub async fn latest(&self, outline: &OutlineId) -> Result<Standing<Outline>, OutlinesError> {
        Ok(self.events.latest(&AggregateId::from(outline)).await?)
    }

    pub fn new_section(&self) -> SectionId {
        SectionId::generate(self.clock.now())
    }

    pub async fn carry_out(
        &self,
        transaction: &mut Transaction,
        outline: &OutlineId,
        command: OutlineCommand,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Appended<OutlineEvent>, OutlinesError> {
        let key = AggregateId::from(outline);

        Ok(match expected {
            Some(expected) => {
                self.events
                    .execute_at(transaction, &key, expected, command, agent)
                    .await?
            }
            None => {
                self.events
                    .execute(transaction, &key, command, agent)
                    .await?
            }
        })
    }

    pub async fn outline_of(
        &self,
        project: &ProjectLink,
    ) -> Result<Option<OutlineId>, OutlinesError> {
        let held = self
            .registry
            .holder(KIND.as_str(), project.as_str())
            .await?;

        Ok(held.map(|held| held.parse()).transpose()?)
    }
}

pub fn not_found(outline: &OutlineId) -> OutlinesError {
    OutlinesError::Events(ServiceError::NotFound {
        aggregate: AggregateId::from(outline),
        kind: KIND,
    })
}
