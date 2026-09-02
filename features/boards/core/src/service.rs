use std::sync::Arc;

use clock::Clock;
use eventsourcing::{
    Agent, AggregateId, EventPublisher, EventSourcingService, EventStore, ServiceError, Standing,
    Version,
};
use ids::InvalidId;
use thiserror::Error;

use crate::board::{Board, BoardCommand, BoardError, BoardEvent, PieceLink, ProjectLink};
use crate::catalog::{BoardCatalog, CatalogError};
use crate::id::BoardId;
use crate::spot::Spot;

#[derive(Debug, Error)]
pub enum BoardServiceError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    Events(#[from] ServiceError<BoardError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

#[derive(Clone)]
pub struct BoardService {
    events: Arc<EventSourcingService<Board>>,
    catalog: Arc<dyn BoardCatalog>,
    clock: Arc<dyn Clock>,
}

pub struct Open {
    pub id: BoardId,
    pub standing: Standing<Board>,
}

impl BoardService {
    pub fn new(
        store: Arc<dyn EventStore<BoardEvent>>,
        catalog: Arc<dyn BoardCatalog>,
        publishing: Arc<dyn EventPublisher<BoardEvent>>,
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

    pub async fn open(&self, project: &str, agent: &Agent) -> Result<Open, BoardServiceError> {
        let id = match self.first_of(project).await? {
            Some(found) => found,
            None => self.start(project, agent).await?,
        };

        Ok(Open {
            id,
            standing: self.events.latest(&AggregateId::from(&id)).await?,
        })
    }

    pub async fn get(&self, board: &str) -> Result<Standing<Board>, BoardServiceError> {
        let board: BoardId = board.parse()?;

        Ok(self.events.latest(&AggregateId::from(&board)).await?)
    }

    pub async fn pin(
        &self,
        board: &str,
        piece: PieceLink,
        at: Spot,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, BoardServiceError> {
        self.carry_out(board, BoardCommand::Pin { piece, at }, expected, agent)
            .await
    }

    pub async fn move_piece(
        &self,
        board: &str,
        piece: PieceLink,
        to: Spot,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, BoardServiceError> {
        self.carry_out(board, BoardCommand::Move { piece, to }, expected, agent)
            .await
    }

    pub async fn unpin(
        &self,
        board: &str,
        piece: PieceLink,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, BoardServiceError> {
        self.carry_out(board, BoardCommand::Unpin { piece }, expected, agent)
            .await
    }

    async fn first_of(&self, project: &str) -> Result<Option<BoardId>, BoardServiceError> {
        let opened = self.catalog.in_project(&ProjectLink::from(project)).await?;

        Ok(opened.first().map(|board| board.id))
    }

    async fn start(&self, project: &str, agent: &Agent) -> Result<BoardId, BoardServiceError> {
        let id = BoardId::generate(self.clock.now());
        self.events
            .begin(
                &AggregateId::from(&id),
                BoardCommand::Start {
                    project: ProjectLink::from(project),
                },
                agent,
            )
            .await?;

        Ok(id)
    }

    async fn carry_out(
        &self,
        board: &str,
        command: BoardCommand,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, BoardServiceError> {
        let board: BoardId = board.parse()?;
        let key = AggregateId::from(&board);

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
