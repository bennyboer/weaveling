use std::sync::Arc;

use clock::Clock;
use eventsourcing::{Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Version};
use thiserror::Error;

use crate::id::{InvalidPieceId, PieceId};
use crate::piece::{PassageLink, Piece, PieceCommand, PieceError, PieceEvent, ProjectLink};
use crate::title::{InvalidPieceTitle, PieceTitle};

#[derive(Debug, Error)]
pub enum PieceServiceError {
    #[error(transparent)]
    InvalidId(#[from] InvalidPieceId),
    #[error(transparent)]
    InvalidTitle(#[from] InvalidPieceTitle),
    #[error(transparent)]
    Events(#[from] ServiceError<PieceError>),
}

#[derive(Clone)]
pub struct PieceService {
    events: Arc<EventSourcingService<Piece>>,
    clock: Arc<dyn Clock>,
}

impl PieceService {
    pub fn new(store: Arc<dyn EventStore<PieceEvent>>, clock: Arc<dyn Clock>) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(store, clock.clone())),
            clock,
        }
    }

    pub async fn capture(
        &self,
        project: &str,
        title: &str,
        agent: &Agent,
    ) -> Result<PieceId, PieceServiceError> {
        let id = PieceId::generate(self.clock.now());

        self.events
            .execute(
                &AggregateId::from(&id),
                PieceCommand::Capture {
                    project: ProjectLink::from(project),
                    title: PieceTitle::new(title)?,
                },
                agent,
            )
            .await?;

        Ok(id)
    }

    pub async fn get(&self, id: &str) -> Result<Piece, PieceServiceError> {
        let id: PieceId = id.parse()?;

        Ok(self.events.latest(&AggregateId::from(&id)).await?)
    }

    pub async fn retitle(
        &self,
        id: &str,
        title: &str,
        agent: &Agent,
    ) -> Result<Version, PieceServiceError> {
        self.carry_out(id, PieceCommand::Retitle(PieceTitle::new(title)?), agent)
            .await
    }

    pub async fn attach_passage(
        &self,
        id: &str,
        passage: &str,
        agent: &Agent,
    ) -> Result<Version, PieceServiceError> {
        self.carry_out(
            id,
            PieceCommand::AttachPassage(PassageLink::from(passage)),
            agent,
        )
        .await
    }

    pub async fn discard(&self, id: &str, agent: &Agent) -> Result<Version, PieceServiceError> {
        self.carry_out(id, PieceCommand::Discard, agent).await
    }

    async fn carry_out(
        &self,
        id: &str,
        command: PieceCommand,
        agent: &Agent,
    ) -> Result<Version, PieceServiceError> {
        let id: PieceId = id.parse()?;

        Ok(self
            .events
            .execute(&AggregateId::from(&id), command, agent)
            .await?)
    }
}
