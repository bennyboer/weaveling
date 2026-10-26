use std::sync::Arc;

use clock::Clock;
use eventsourcing::{
    Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing, StoreError,
    Version,
};
use registry::Registry;

use crate::board::{Board, BoardCommand, BoardEvent, KIND, ProjectLink};
use crate::error::BoardsError;
use crate::id::BoardId;

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

    pub async fn open(&self, project: &ProjectLink, agent: &Agent) -> Result<Open, BoardsError> {
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

    pub async fn latest(&self, board: &BoardId) -> Result<Standing<Board>, BoardsError> {
        Ok(self.events.latest(&AggregateId::from(board)).await?)
    }

    pub async fn carry_out(
        &self,
        board: &BoardId,
        command: BoardCommand,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, BoardsError> {
        let key = AggregateId::from(board);

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

    pub async fn board_of(&self, project: &ProjectLink) -> Result<Option<BoardId>, BoardsError> {
        let held = self
            .registry
            .holder(KIND.as_str(), project.as_str())
            .await?;

        Ok(held.map(|held| held.parse()).transpose()?)
    }

    async fn claimed_by(&self, project: &ProjectLink) -> Result<BoardId, BoardsError> {
        let mine = BoardId::generate(self.clock.now());
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
    ) -> Result<(), BoardsError> {
        match self
            .events
            .begin(
                key,
                BoardCommand::Start {
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

pub fn not_found(board: &BoardId) -> BoardsError {
    BoardsError::Events(ServiceError::NotFound {
        aggregate: AggregateId::from(board),
        kind: KIND,
    })
}
