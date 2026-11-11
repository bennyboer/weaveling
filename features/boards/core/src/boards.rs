use std::sync::Arc;

use authorizing::{AccessWatch, Act, Authorizer};
use belonging::Belongings;
use clock::Clock;
use eventsourcing::{Agent, EventStore, ServiceError, Standing, Version};
use registry::Registry;
use time::OffsetDateTime;
use transacting::{Transaction, Transactions};

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
    belongings: Arc<dyn Belongings>,
    authorizer: Arc<dyn Authorizer>,
}

const IDEA: &str = "idea";
const PROJECT: &str = "project";

impl BoardsCore {
    pub fn new(
        store: Arc<dyn EventStore<BoardEvent>>,
        transactions: Arc<dyn Transactions>,
        catalog: Arc<dyn BoardCatalog>,
        registry: Arc<dyn Registry>,
        belongings: Arc<dyn Belongings>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: BoardService::new(store, registry, clock.clone()),
            transactions,
            clock,
            catalog,
            belongings,
            authorizer,
        }
    }

    pub async fn open(&self, project: &str, agent: &Agent) -> Result<Open, BoardsError> {
        let project = ProjectLink::from(project);
        self.authorized(agent, Act::Read, &project, || BoardsError::ProjectNotThere)
            .await?;
        let id = transacting::retrying!(&*self.transactions, |transaction| {
            if self
                .catalog
                .has_project_tombstone_within(transaction, &project)
                .await?
            {
                return Err(BoardsError::ProjectNotThere);
            }
            self.service.claim(transaction, &project, agent).await
        })?;

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
        let board: BoardId = board.parse()?;
        let project = self.authorized_to_write(&board, agent).await?;
        if self
            .belongings
            .owner_of(IDEA, idea.as_str(), PROJECT)
            .await?
            .is_some_and(|known| known != project.as_str())
        {
            return Err(BoardsError::IdeaNotThere);
        }

        self.carried_out(
            &board,
            BoardCommand::Pin { idea, at, size },
            expected,
            agent,
        )
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

    pub async fn catalogue(
        &self,
        within: Option<&mut Transaction>,
        board: &BoardId,
    ) -> Result<(), BoardsError> {
        let standing = self.service.latest(board).await?;

        transacting::joining!(&*self.transactions, within, |transaction| {
            if standing.state.is_discarded() {
                self.catalog
                    .forget(transaction, board, self.clock.now())
                    .await?;
            } else {
                self.catalog
                    .remember(transaction, &BoardSummary::of(*board, &standing.state))
                    .await?;
            }

            Ok(())
        })
    }

    pub async fn index_pins(
        &self,
        within: Option<&mut Transaction>,
        board: &BoardId,
    ) -> Result<(), BoardsError> {
        let standing = self.service.latest(board).await?;
        let holding: Vec<IdeaLink> = if standing.state.is_discarded() {
            Vec::new()
        } else {
            pinned(&standing.state)
        };

        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog.holds(transaction, *board, &holding).await?;

            Ok(())
        })
    }

    /// Keeps which project an idea is of, so that no board of another project may pin it.
    pub async fn idea_captured(
        &self,
        within: Option<&mut Transaction>,
        idea: &str,
        project: &str,
    ) -> Result<(), BoardsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.belongings
                .remember(transaction, IDEA, idea, PROJECT, project)
                .await?;

            Ok(())
        })
    }

    pub async fn lay_tombstone(
        &self,
        within: Option<&mut Transaction>,
        idea: &str,
        at: OffsetDateTime,
    ) -> Result<(), BoardsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .lay_tombstone(transaction, &IdeaLink::from(idea), at)
                .await?;
            self.belongings.forget(transaction, IDEA, idea).await?;

            Ok(())
        })
    }

    pub async fn let_go_everywhere(
        &self,
        within: Option<&mut Transaction>,
        idea: &str,
    ) -> Result<(), BoardsError> {
        let gone = IdeaLink::from(idea);

        transacting::joining!(&*self.transactions, within, |transaction| {
            for board in self.catalog.boards_holding(&gone).await? {
                self.let_go(transaction, board, &gone).await?;
            }

            Ok(())
        })
    }

    pub async fn let_go_of_the_gone(
        &self,
        within: Option<&mut Transaction>,
        board: &BoardId,
    ) -> Result<(), BoardsError> {
        let still_pinned = pinned(&self.service.latest(board).await?.state);
        let Some(gone) = self
            .catalog
            .held_though_tombstoned(*board)
            .await?
            .into_iter()
            .find(|idea| still_pinned.contains(idea))
        else {
            return Ok(());
        };

        transacting::joining!(&*self.transactions, within, |transaction| {
            self.let_go(transaction, *board, &gone).await?;

            Ok(())
        })
    }

    pub async fn sweep(&self, project: &str) -> Result<(), BoardsError> {
        let Some(board) = self.service.board_of(&ProjectLink::from(project)).await? else {
            return Ok(());
        };

        self.discarded(&board).await
    }

    async fn discarded(&self, board: &BoardId) -> Result<(), BoardsError> {
        match self.as_the_system(board, BoardCommand::Discard).await {
            Ok(())
            | Err(BoardsError::Events(ServiceError::Refused(BoardError::Discarded)))
            | Err(BoardsError::Events(ServiceError::NotFound { .. })) => Ok(()),
            Err(refused) => Err(refused),
        }
    }

    pub async fn lay_project_tombstone(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        at: OffsetDateTime,
    ) -> Result<(), BoardsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .lay_project_tombstone(transaction, &ProjectLink::from(project), at)
                .await?;

            Ok(())
        })
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, BoardsError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            let forgotten = self
                .catalog
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;

            Ok(forgotten)
        })
    }

    async fn let_go(
        &self,
        transaction: &mut Transaction,
        board: BoardId,
        gone: &IdeaLink,
    ) -> Result<(), BoardsError> {
        self.authorized_to_write(&board, &Agent::System).await?;
        match self
            .service
            .carry_out(
                transaction,
                &board,
                BoardCommand::Unpin { idea: gone.clone() },
                None,
                &Agent::System,
            )
            .await
        {
            Ok(_)
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

        self.carried_out(&board, command, expected, agent).await
    }

    async fn carried_out(
        &self,
        board: &BoardId,
        command: BoardCommand,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Standing<Board>, BoardsError> {
        transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                self.service
                    .carry_out(transaction, board, command.clone(), expected, agent)
                    .await
            }
        )?;

        self.service.latest(board).await
    }

    async fn authorized_to_write(
        &self,
        board: &BoardId,
        agent: &Agent,
    ) -> Result<ProjectLink, BoardsError> {
        let standing = self.service.latest(board).await?;
        let project = standing.state.project().clone();

        self.authorized(agent, Act::Write, &project, || service::not_found(board))
            .await?;

        Ok(project)
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

fn pinned(board: &Board) -> Vec<IdeaLink> {
    board
        .ideas()
        .into_iter()
        .map(|positioned| positioned.idea)
        .collect()
}
