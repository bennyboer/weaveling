use std::sync::Arc;

use clock::Clock;
use eventsourcing::{
    Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing, StoreError,
    Version,
};
use ids::InvalidId;
use registry::{Registry, RegistryError};
use thiserror::Error;

use crate::catalog::CatalogError;
use crate::id::{OutlineId, SectionId};
use crate::outline::{
    KIND, Outline, OutlineCommand, OutlineError, OutlineEvent, PieceLink, ProjectLink,
};
use crate::title::SectionTitle;

#[derive(Debug, Error)]
pub enum OutlineServiceError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    Events(#[from] ServiceError<OutlineError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Registry(#[from] RegistryError),
}

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

    pub async fn open(&self, project: &str, agent: &Agent) -> Result<Open, OutlineServiceError> {
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

    pub async fn get(&self, outline: &str) -> Result<Standing<Outline>, OutlineServiceError> {
        let outline: OutlineId = outline.parse()?;

        Ok(self.events.latest(&AggregateId::from(&outline)).await?)
    }

    pub async fn add(
        &self,
        outline: &str,
        under: Option<SectionId>,
        after: Option<SectionId>,
        title: SectionTitle,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Added, OutlineServiceError> {
        let section = SectionId::generate(self.clock.now());
        let version = self
            .carry_out(
                outline,
                OutlineCommand::Add {
                    section,
                    under,
                    after,
                    title,
                },
                expected,
                agent,
            )
            .await?;

        Ok(Added { section, version })
    }

    pub async fn retitle(
        &self,
        outline: &str,
        section: SectionId,
        title: SectionTitle,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, OutlineServiceError> {
        self.carry_out(
            outline,
            OutlineCommand::Retitle { section, title },
            expected,
            agent,
        )
        .await
    }

    pub async fn place(
        &self,
        outline: &str,
        section: SectionId,
        under: Option<SectionId>,
        after: Option<SectionId>,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, OutlineServiceError> {
        self.carry_out(
            outline,
            OutlineCommand::Move {
                section,
                under,
                after,
            },
            expected,
            agent,
        )
        .await
    }

    pub async fn promote(
        &self,
        outline: &str,
        section: SectionId,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, OutlineServiceError> {
        self.carry_out(
            outline,
            OutlineCommand::Promote { section },
            expected,
            agent,
        )
        .await
    }

    pub async fn demote(
        &self,
        outline: &str,
        section: SectionId,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, OutlineServiceError> {
        self.carry_out(outline, OutlineCommand::Demote { section }, expected, agent)
            .await
    }

    pub async fn remove(
        &self,
        outline: &str,
        section: SectionId,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, OutlineServiceError> {
        self.carry_out(outline, OutlineCommand::Remove { section }, expected, agent)
            .await
    }

    pub async fn attach(
        &self,
        outline: &str,
        piece: PieceLink,
        to: SectionId,
        after: Option<PieceLink>,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, OutlineServiceError> {
        self.carry_out(
            outline,
            OutlineCommand::AttachPiece { piece, to, after },
            expected,
            agent,
        )
        .await
    }

    pub async fn detach(
        &self,
        outline: &str,
        piece: PieceLink,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, OutlineServiceError> {
        self.carry_out(
            outline,
            OutlineCommand::DetachPiece { piece },
            expected,
            agent,
        )
        .await
    }

    async fn claimed_by(&self, project: &str) -> Result<OutlineId, OutlineServiceError> {
        let mine = OutlineId::generate(self.clock.now());
        let held = self
            .registry
            .claim(KIND.as_str(), project, &mine.to_string())
            .await?;

        Ok(held.parse()?)
    }

    async fn start(
        &self,
        key: &AggregateId,
        project: &str,
        agent: &Agent,
    ) -> Result<(), OutlineServiceError> {
        match self
            .events
            .begin(
                key,
                OutlineCommand::Start {
                    project: ProjectLink::from(project),
                },
                agent,
            )
            .await
        {
            Ok(_) | Err(ServiceError::Store(StoreError::Outdated { .. })) => Ok(()),
            Err(refused) => Err(refused.into()),
        }
    }

    async fn carry_out(
        &self,
        outline: &str,
        command: OutlineCommand,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, OutlineServiceError> {
        let outline: OutlineId = outline.parse()?;
        let key = AggregateId::from(&outline);

        let landed = match expected {
            Some(expected) => {
                self.events
                    .execute_at(&key, expected, command, agent)
                    .await?
            }
            None => self.events.execute(&key, command, agent).await?,
        };

        Ok(landed.version)
    }
}
