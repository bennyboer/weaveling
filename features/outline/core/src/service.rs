use std::sync::Arc;

use clock::Clock;
use eventsourcing::{
    Agent, AggregateId, Appended, EventSourcingService, EventStore, ServiceError, Standing,
    StoreError, Version,
};
use registry::Registry;

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

fn attachments_changed_in(landed: &Appended<OutlineEvent>) -> Vec<SectionId> {
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

    pub async fn open(&self, project: &ProjectLink, agent: &Agent) -> Result<Open, OutlinesError> {
        let id = self.claimed_by(project).await?;
        let key = AggregateId::from(&id);

        let standing = match self.events.latest(&key).await {
            Ok(standing) => standing,
            Err(ServiceError::NotFound { .. }) => {
                self.start(&key, project, agent).await?;

                self.events.latest(&key).await?
            }
            Err(refused) => return Err(refused.into()),
        };

        Ok(Open { id, standing })
    }

    pub async fn latest(&self, outline: &OutlineId) -> Result<Standing<Outline>, OutlinesError> {
        Ok(self.events.latest(&AggregateId::from(outline)).await?)
    }

    pub fn new_section(&self) -> SectionId {
        SectionId::generate(self.clock.now())
    }

    pub async fn carry_out(
        &self,
        outline: &OutlineId,
        command: OutlineCommand,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Changed, OutlinesError> {
        let key = AggregateId::from(outline);

        let landed = match expected {
            Some(expected) => {
                self.events
                    .execute_at(&key, expected, command, agent)
                    .await?
            }
            None => self.events.execute(&key, command, agent).await?,
        };

        Ok(Changed {
            version: landed.version,
            attachments_changed_in: attachments_changed_in(&landed),
            standing: self.latest(outline).await?,
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

    async fn claimed_by(&self, project: &ProjectLink) -> Result<OutlineId, OutlinesError> {
        let mine = OutlineId::generate(self.clock.now());
        let held = self
            .registry
            .claim(KIND.as_str(), project.as_str(), &mine.to_string())
            .await?;

        Ok(held.parse()?)
    }

    async fn start(
        &self,
        key: &AggregateId,
        project: &ProjectLink,
        agent: &Agent,
    ) -> Result<(), OutlinesError> {
        match self
            .events
            .begin(
                key,
                OutlineCommand::Start {
                    project: project.clone(),
                },
                agent,
            )
            .await
        {
            Ok(_) | Err(ServiceError::Store(StoreError::Outdated { .. })) => Ok(()),
            Err(refused) => Err(refused.into()),
        }
    }
}

pub fn not_found(outline: &OutlineId) -> OutlinesError {
    OutlinesError::Events(ServiceError::NotFound {
        aggregate: AggregateId::from(outline),
        kind: KIND,
    })
}
