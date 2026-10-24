use boards_core::{BoardError, BoardService, BoardServiceError, ProjectLink};
use eventsourcing::{Agent, ServiceError};
use messaging::{Failure, Handler, Message};
use projects_contract::{DELETED, ProjectEventDTO};

pub struct DiscardBoardsOnProjectDeletedListener {
    boards: BoardService,
}

impl DiscardBoardsOnProjectDeletedListener {
    pub fn new(boards: BoardService) -> Self {
        Self { boards }
    }

    async fn discard_everything(&self, project: &ProjectLink) -> Result<(), Failure> {
        let Some(board) = self.boards.board_of(project.as_str()).await? else {
            return Ok(());
        };

        match self
            .boards
            .discard(&board.to_string(), None, &nobody())
            .await
        {
            Ok(_) => Ok(()),
            Err(refused) if already_discarded(&refused) => Ok(()),
            Err(refused) => Err(refused.into()),
        }
    }
}

fn already_discarded(refused: &BoardServiceError) -> bool {
    matches!(
        refused,
        BoardServiceError::Events(ServiceError::Refused(BoardError::Discarded))
    )
}

fn nobody() -> Agent {
    Agent::System
}

#[async_trait::async_trait]
impl Handler for DiscardBoardsOnProjectDeletedListener {
    const NAME: &'static str = "discard-boards-of-deleted-project";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str = "The board of a deleted project was not cleared away.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let deleted = eventpublishing::published_in::<ProjectEventDTO>(message)?;

        self.discard_everything(&ProjectLink::from(deleted.aggregate.id.as_str()))
            .await
    }
}
