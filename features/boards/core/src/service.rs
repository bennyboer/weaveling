use std::sync::Arc;

use clock::Clock;
use eventsourcing::{
    Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing, Version,
};
use registry::Registry;
use transacting::Transaction;

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

    pub async fn claim(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        agent: &Agent,
    ) -> Result<BoardId, BoardsError> {
        let mine = BoardId::generate(self.clock.now());
        let held = self
            .registry
            .claim(
                transaction,
                KIND.as_str(),
                project.as_str(),
                &mine.to_string(),
            )
            .await?;
        let id: BoardId = held.parse()?;
        if id == mine {
            self.events
                .begin(
                    transaction,
                    &AggregateId::from(&id),
                    BoardCommand::Start {
                        project: project.clone(),
                    },
                    agent,
                )
                .await?;
        }

        Ok(id)
    }

    pub async fn latest(&self, board: &BoardId) -> Result<Standing<Board>, BoardsError> {
        Ok(self.events.latest(&AggregateId::from(board)).await?)
    }

    pub async fn carry_out(
        &self,
        transaction: &mut Transaction,
        board: &BoardId,
        command: BoardCommand,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, BoardsError> {
        Ok(self
            .events
            .execute_at(
                transaction,
                &AggregateId::from(board),
                expected,
                command,
                agent,
            )
            .await?
            .version)
    }

    pub async fn board_of(&self, project: &ProjectLink) -> Result<Option<BoardId>, BoardsError> {
        let held = self
            .registry
            .holder(KIND.as_str(), project.as_str())
            .await?;

        Ok(held.map(|held| held.parse()).transpose()?)
    }
}

pub fn not_found(board: &BoardId) -> BoardsError {
    BoardsError::Events(ServiceError::NotFound {
        aggregate: AggregateId::from(board),
        kind: KIND,
    })
}
