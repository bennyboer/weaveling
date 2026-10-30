use std::sync::Arc;

use authorizing::{AccessWatch, Act, Authorizer};
use clock::Clock;
use eventsourcing::{Agent, EventStore, ServiceError, Standing, Version};
use registry::Registry;
use time::OffsetDateTime;

use crate::board::{Board, BoardCommand, BoardError, BoardEvent, IdeaLink, ProjectLink};
use crate::catalog::{BoardCatalog, BoardSummary};
use crate::error::BoardsError;
use crate::id::BoardId;
use crate::service::{self, BoardService, Open};
use crate::size::Size;
use crate::spot::Spot;

#[derive(Clone)]
pub struct BoardsCore {
    service: BoardService,
    catalog: Arc<dyn BoardCatalog>,
    authorizer: Arc<dyn Authorizer>,
}

impl BoardsCore {
    pub fn new(
        store: Arc<dyn EventStore<BoardEvent>>,
        catalog: Arc<dyn BoardCatalog>,
        registry: Arc<dyn Registry>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: BoardService::new(store, registry, clock),
            catalog,
            authorizer,
        }
    }

    pub async fn open(&self, project: &str, agent: &Agent) -> Result<Open, BoardsError> {
        let project = ProjectLink::from(project);
        self.authorized(agent, Act::Read, &project, || BoardsError::ProjectNotThere)
            .await?;

        self.service.open(&project, agent).await
    }

    pub async fn read(&self, board: &str, agent: &Agent) -> Result<Standing<Board>, BoardsError> {
        let board: BoardId = board.parse()?;
        let standing = self.service.latest(&board).await?;
        self.authorized(agent, Act::Read, standing.state.project(), || {
            service::not_found(&board)
        })
        .await?;

        Ok(standing)
    }

    pub async fn watch_access(&self, board: &str) -> Result<AccessWatch, BoardsError> {
        let board: BoardId = board.parse()?;
        let standing = self.service.latest(&board).await?;

        Ok(self
            .authorizer
            .watch_access(standing.state.project().as_str()))
    }

    pub async fn pin(
        &self,
        board: &str,
        idea: IdeaLink,
        at: Spot,
        size: Size,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Standing<Board>, BoardsError> {
        self.written(board, BoardCommand::Pin { idea, at, size }, expected, agent)
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
    ) -> Result<Standing<Board>, BoardsError> {
        self.written(
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
    ) -> Result<Standing<Board>, BoardsError> {
        self.written(board, BoardCommand::Unpin { idea }, expected, agent)
            .await
    }

    pub async fn catalogue(&self, board: &BoardId) -> Result<(), BoardsError> {
        let standing = self.service.latest(board).await?;

        if standing.state.is_discarded() {
            self.catalog.forget(board).await?;
        } else {
            self.catalog
                .remember(&BoardSummary::of(*board, &standing.state))
                .await?;
        }

        Ok(())
    }

    pub async fn index_pins(&self, board: &BoardId) -> Result<(), BoardsError> {
        let standing = self.service.latest(board).await?;
        if standing.state.is_discarded() {
            self.catalog.holds(*board, &[]).await?;
            return Ok(());
        }

        let holding: Vec<IdeaLink> = standing
            .state
            .ideas()
            .into_iter()
            .map(|positioned| positioned.idea)
            .collect();
        self.catalog.holds(*board, &holding).await?;

        for gone in self.catalog.held_though_tombstoned(*board).await? {
            self.let_go(*board, &gone).await?;
        }

        Ok(())
    }

    pub async fn let_go_everywhere(
        &self,
        idea: &str,
        at: OffsetDateTime,
    ) -> Result<(), BoardsError> {
        let gone = IdeaLink::from(idea);
        self.catalog.lay_tombstone(&gone, at).await?;

        for board in self.catalog.boards_holding(&gone).await? {
            self.let_go(board, &gone).await?;
        }

        Ok(())
    }

    pub async fn sweep(&self, project: &str) -> Result<(), BoardsError> {
        let Some(board) = self.service.board_of(&ProjectLink::from(project)).await? else {
            return Ok(());
        };

        match self.as_the_system(&board, BoardCommand::Discard).await {
            Ok(()) | Err(BoardsError::Events(ServiceError::Refused(BoardError::Discarded))) => {
                Ok(())
            }
            Err(refused) => Err(refused),
        }
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, BoardsError> {
        Ok(self
            .catalog
            .forget_tombstones_laid_before(before, at_most)
            .await?)
    }

    async fn let_go(&self, board: BoardId, gone: &IdeaLink) -> Result<(), BoardsError> {
        match self
            .as_the_system(&board, BoardCommand::Unpin { idea: gone.clone() })
            .await
        {
            Ok(())
            | Err(BoardsError::Events(ServiceError::Refused(
                BoardError::NotPinned | BoardError::Discarded,
            ))) => Ok(()),
            Err(refused) => Err(refused),
        }
    }

    async fn as_the_system(
        &self,
        board: &BoardId,
        command: BoardCommand,
    ) -> Result<(), BoardsError> {
        self.authorized_to_write(board, &Agent::System).await?;
        self.service
            .carry_out(board, command, None, &Agent::System)
            .await?;

        Ok(())
    }

    async fn written(
        &self,
        board: &str,
        command: BoardCommand,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Standing<Board>, BoardsError> {
        let board: BoardId = board.parse()?;
        self.authorized_to_write(&board, agent).await?;
        self.service
            .carry_out(&board, command, expected, agent)
            .await?;

        self.service.latest(&board).await
    }

    async fn authorized_to_write(&self, board: &BoardId, agent: &Agent) -> Result<(), BoardsError> {
        let standing = self.service.latest(board).await?;

        self.authorized(agent, Act::Write, standing.state.project(), || {
            service::not_found(board)
        })
        .await
    }

    async fn authorized(
        &self,
        agent: &Agent,
        act: Act,
        project: &ProjectLink,
        otherwise: impl FnOnce() -> BoardsError,
    ) -> Result<(), BoardsError> {
        if self.authorizer.may(agent, act, project.as_str()).await? {
            Ok(())
        } else if self
            .authorizer
            .may(agent, Act::Read, project.as_str())
            .await?
        {
            Err(BoardsError::NotAnAuthor)
        } else {
            Err(otherwise())
        }
    }
}
