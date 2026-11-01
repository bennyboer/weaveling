use std::sync::Arc;

use authorizing::{AccessWatch, Act, Authorizer};
use clock::Clock;
use eventsourcing::{Agent, EventStore, ServiceError, Standing, Version};
use registry::Registry;
use time::OffsetDateTime;
use transacting::Transactions;

use crate::board::{Board, BoardCommand, BoardError, BoardEvent, IdeaLink, ProjectLink};
use crate::catalog::{BoardCatalog, BoardSummary};
use crate::error::BoardsError;
use crate::id::BoardId;
use crate::service::{self, BoardService, Open};
use crate::size::Size;
use crate::spot::Spot;

#[derive(Clone)]
pub struct BoardsCore {
    clock: Arc<dyn Clock>,
    service: BoardService,
    transactions: Arc<dyn Transactions>,
    catalog: Arc<dyn BoardCatalog>,
    authorizer: Arc<dyn Authorizer>,
}

impl BoardsCore {
    pub fn new(
        store: Arc<dyn EventStore<BoardEvent>>,
        transactions: Arc<dyn Transactions>,
        catalog: Arc<dyn BoardCatalog>,
        registry: Arc<dyn Registry>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: BoardService::new(store, transactions.clone(), registry, clock.clone()),
            transactions,
            clock,
            catalog,
            authorizer,
        }
    }

    pub async fn open(&self, project: &str, agent: &Agent) -> Result<Open, BoardsError> {
        let project = ProjectLink::from(project);
        self.authorized(agent, Act::Read, &project, || BoardsError::ProjectNotThere)
            .await?;

        let mut transaction = self.transactions.begin().await?;
        let id = self
            .service
            .claim(&mut transaction, &project, agent)
            .await?;
        transaction.finish().await?;

        Ok(Open {
            id,
            standing: self.service.latest(&id).await?,
        })
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

        let mut transaction = self.transactions.begin().await?;
        if standing.state.is_discarded() {
            self.catalog
                .forget(&mut transaction, board, self.clock.now())
                .await?;
        } else {
            self.catalog
                .remember(&mut transaction, &BoardSummary::of(*board, &standing.state))
                .await?;
        }

        Ok(transaction.finish().await?)
    }

    pub async fn index_pins(&self, board: &BoardId) -> Result<(), BoardsError> {
        let standing = self.service.latest(board).await?;
        let holding: Vec<IdeaLink> = if standing.state.is_discarded() {
            Vec::new()
        } else {
            standing
                .state
                .ideas()
                .into_iter()
                .map(|positioned| positioned.idea)
                .collect()
        };

        let mut transaction = self.transactions.begin().await?;
        self.catalog
            .holds(&mut transaction, *board, &holding)
            .await?;
        transaction.finish().await?;
        if standing.state.is_discarded() {
            return Ok(());
        }

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
        let mut transaction = self.transactions.begin().await?;
        self.catalog
            .lay_tombstone(&mut transaction, &gone, at)
            .await?;
        transaction.finish().await?;

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
        let mut transaction = self.transactions.begin().await?;
        let forgotten = self
            .catalog
            .forget_tombstones_laid_before(&mut transaction, before, at_most)
            .await?;
        transaction.finish().await?;

        Ok(forgotten)
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
        transacting::retrying!(&*self.transactions, |transaction| {
            self.service
                .carry_out(transaction, board, command.clone(), None, &Agent::System)
                .await
        })?;

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
        transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                self.service
                    .carry_out(transaction, &board, command.clone(), expected, agent)
                    .await
            }
        )?;

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
