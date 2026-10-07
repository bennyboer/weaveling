use std::sync::Arc;

use clock::Clock;
use eventsourcing::{
    Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing, StoreError,
    Version,
};
use ids::InvalidId;
use registry::{Registry, RegistryError};
use thiserror::Error;

use crate::board::{Board, BoardCommand, BoardError, BoardEvent, IdeaLink, KIND, ProjectLink};
use crate::catalog::CatalogError;
use crate::id::BoardId;
use crate::size::Size;
use crate::spot::Spot;

#[derive(Debug, Error)]
pub enum BoardServiceError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    Events(#[from] ServiceError<BoardError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Registry(#[from] RegistryError),
}

#[derive(Clone)]
pub struct BoardService {
    events: Arc<EventSourcingService<Board>>,
    registry: Arc<dyn Registry>,
    clock: Arc<dyn Clock>,
}

pub struct Open {
    pub id: BoardId,
    pub standing: Standing<Board>,
}

impl BoardService {
    pub fn new(
        store: Arc<dyn EventStore<BoardEvent>>,
        registry: Arc<dyn Registry>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(store, clock.clone())),
            registry,
            clock,
        }
    }

    pub async fn open(&self, project: &str, agent: &Agent) -> Result<Open, BoardServiceError> {
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

    pub async fn get(&self, board: &str) -> Result<Standing<Board>, BoardServiceError> {
        let board: BoardId = board.parse()?;

        Ok(self.events.latest(&AggregateId::from(&board)).await?)
    }

    pub async fn pin(
        &self,
        board: &str,
        idea: IdeaLink,
        at: Spot,
        size: Size,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, BoardServiceError> {
        self.carry_out(board, BoardCommand::Pin { idea, at, size }, expected, agent)
            .await
    }

    pub async fn reshape(
        &self,
        board: &str,
        idea: IdeaLink,
        to: Option<Spot>,
        size: Option<Size>,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, BoardServiceError> {
        self.carry_out(
            board,
            BoardCommand::Reshape { idea, to, size },
            expected,
            agent,
        )
        .await
    }

    pub async fn unpin(
        &self,
        board: &str,
        idea: IdeaLink,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, BoardServiceError> {
        self.carry_out(board, BoardCommand::Unpin { idea }, expected, agent)
            .await
    }

    pub async fn discard(
        &self,
        board: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, BoardServiceError> {
        self.carry_out(board, BoardCommand::Discard, expected, agent)
            .await
    }

    pub async fn board_of(&self, project: &str) -> Result<Option<BoardId>, BoardServiceError> {
        let held = self.registry.holder(KIND.as_str(), project).await?;

        Ok(held.map(|held| held.parse()).transpose()?)
    }

    async fn claimed_by(&self, project: &str) -> Result<BoardId, BoardServiceError> {
        let mine = BoardId::generate(self.clock.now());
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
    ) -> Result<(), BoardServiceError> {
        match self
            .events
            .begin(
                key,
                BoardCommand::Start {
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
