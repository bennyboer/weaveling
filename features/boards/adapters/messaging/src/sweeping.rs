use boards_core::{BoardError, BoardService, BoardServiceError, ProjectLink};
use eventpublishing::{UnreadableMessage, published_in};
use eventsourcing::{Agent, ServiceError};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use projects_contract::{DELETED, ProjectEventDTO};
use thiserror::Error;

const NAME: &str = "discard-boards-of-deleted-project";

pub struct DiscardBoardsOnProjectDeleted {
    boards: BoardService,
}

#[derive(Debug, Error)]
enum NotSwept {
    #[error(transparent)]
    Unreadable(#[from] UnreadableMessage),
    #[error(transparent)]
    Refused(#[from] BoardServiceError),
}

pub fn when_project_deleted() -> Subscription {
    Subscription::parse(DELETED).expect("a declared routing key holds no wildcards")
}

impl DiscardBoardsOnProjectDeleted {
    pub fn new(boards: BoardService) -> Self {
        Self { boards }
    }

    async fn discard_everything(&self, project: &ProjectLink) -> Result<(), NotSwept> {
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

    async fn work_through(&self, message: &Message) -> Result<(), NotSwept> {
        let deleted = published_in::<ProjectEventDTO>(message)?;

        self.discard_everything(&ProjectLink::from(deleted.aggregate.id.as_str()))
            .await
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
impl Listener for DiscardBoardsOnProjectDeleted {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the sweep listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![when_project_deleted()]
    }

    fn when_refused(&self) -> &'static str {
        "The board of a deleted project was not cleared away."
    }

    fn delivery(&self) -> Delivery {
        Delivery::Kept
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        self.work_through(message)
            .await
            .map_err(|why| NotHandled::because(self.named(), message.routing.clone(), why))
    }
}
