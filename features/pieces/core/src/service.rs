use std::sync::Arc;

use clock::Clock;
use eventsourcing::{
    Agent, AggregateId, Appended, EventSourcingService, EventStore, ServiceError, Standing, Version,
};
use thiserror::Error;

use crate::catalog::{CatalogError, PieceCatalog, PieceSummary};
use crate::id::{InvalidPieceId, PieceId};
use crate::piece::{PassageLink, Piece, PieceCommand, PieceError, PieceEvent, ProjectLink};
use crate::publishing::{PieceEventPublisher, PublishError};
use crate::title::{InvalidPieceTitle, PieceTitle};

#[derive(Debug, Error)]
pub enum PieceServiceError {
    #[error(transparent)]
    InvalidId(#[from] InvalidPieceId),
    #[error(transparent)]
    InvalidTitle(#[from] InvalidPieceTitle),
    #[error(transparent)]
    Events(#[from] ServiceError<PieceError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Unpublished(#[from] PublishError),
}

#[derive(Clone)]
pub struct PieceService {
    events: Arc<EventSourcingService<Piece>>,
    catalog: Arc<dyn PieceCatalog>,
    publishing: Arc<dyn PieceEventPublisher>,
    clock: Arc<dyn Clock>,
}

impl PieceService {
    pub fn new(
        store: Arc<dyn EventStore<PieceEvent>>,
        catalog: Arc<dyn PieceCatalog>,
        publishing: Arc<dyn PieceEventPublisher>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(store, clock.clone())),
            catalog,
            publishing,
            clock,
        }
    }

    pub async fn list(&self, project: &str) -> Result<Vec<PieceSummary>, PieceServiceError> {
        Ok(self.catalog.in_project(&ProjectLink::from(project)).await?)
    }

    pub async fn capture(
        &self,
        project: &str,
        title: &str,
        agent: &Agent,
    ) -> Result<PieceId, PieceServiceError> {
        let id = PieceId::generate(self.clock.now());

        let landed = self
            .events
            .begin(
                &AggregateId::from(&id),
                PieceCommand::Capture {
                    project: ProjectLink::from(project),
                    title: PieceTitle::new(title)?,
                },
                agent,
            )
            .await?;
        self.publish(&landed).await?;

        Ok(id)
    }

    pub async fn get(&self, id: &str) -> Result<Standing<Piece>, PieceServiceError> {
        let id: PieceId = id.parse()?;

        Ok(self.events.latest(&AggregateId::from(&id)).await?)
    }

    pub async fn retitle(
        &self,
        id: &str,
        title: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, PieceServiceError> {
        self.carry_out(
            id,
            PieceCommand::Retitle(PieceTitle::new(title)?),
            expected,
            agent,
        )
        .await
    }

    pub async fn attach_passage(
        &self,
        id: &str,
        passage: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, PieceServiceError> {
        self.carry_out(
            id,
            PieceCommand::AttachPassage(PassageLink::from(passage)),
            expected,
            agent,
        )
        .await
    }

    pub async fn discard(
        &self,
        id: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, PieceServiceError> {
        self.carry_out(id, PieceCommand::Discard, expected, agent)
            .await
    }

    async fn carry_out(
        &self,
        id: &str,
        command: PieceCommand,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, PieceServiceError> {
        let id: PieceId = id.parse()?;
        let key = AggregateId::from(&id);

        let landed = match expected {
            Some(expected) => {
                self.events
                    .execute_at(&key, expected, command, agent)
                    .await?
            }
            None => self.events.execute(&key, command, agent).await?,
        };
        self.publish(&landed).await?;

        Ok(landed.version)
    }

    async fn publish(&self, landed: &Appended<PieceEvent>) -> Result<(), PieceServiceError> {
        for happened in &landed.events {
            self.publishing.publish(happened).await?;
        }

        Ok(())
    }
}
