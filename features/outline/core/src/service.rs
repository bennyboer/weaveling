use std::sync::Arc;

use clock::Clock;
use eventsourcing::{
    Agent, AggregateId, EventPublisher, EventSourcingService, EventStore, ServiceError, Standing,
    Version,
};
use ids::InvalidId;
use thiserror::Error;

use crate::catalog::{CatalogError, OutlineCatalog};
use crate::id::{OutlineId, SectionId};
use crate::outline::{Outline, OutlineCommand, OutlineError, OutlineEvent, PieceLink, ProjectLink};
use crate::title::SectionTitle;

#[derive(Debug, Error)]
pub enum OutlineServiceError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    Events(#[from] ServiceError<OutlineError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

#[derive(Clone)]
pub struct OutlineService {
    events: Arc<EventSourcingService<Outline>>,
    catalog: Arc<dyn OutlineCatalog>,
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
        catalog: Arc<dyn OutlineCatalog>,
        publishing: Arc<dyn EventPublisher<OutlineEvent>>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::publishing_to(
                store,
                clock.clone(),
                publishing,
            )),
            catalog,
            clock,
        }
    }

    pub async fn open(&self, project: &str, agent: &Agent) -> Result<Open, OutlineServiceError> {
        let id = match self.first_of(project).await? {
            Some(found) => found,
            None => self.start(project, agent).await?,
        };

        Ok(Open {
            id,
            standing: self.events.latest(&AggregateId::from(&id)).await?,
        })
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

    async fn first_of(&self, project: &str) -> Result<Option<OutlineId>, OutlineServiceError> {
        let opened = self.catalog.in_project(&ProjectLink::from(project)).await?;

        Ok(opened.first().map(|outline| outline.id))
    }

    async fn start(&self, project: &str, agent: &Agent) -> Result<OutlineId, OutlineServiceError> {
        let id = OutlineId::generate(self.clock.now());
        self.events
            .begin(
                &AggregateId::from(&id),
                OutlineCommand::Start {
                    project: ProjectLink::from(project),
                },
                agent,
            )
            .await?;

        Ok(id)
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
