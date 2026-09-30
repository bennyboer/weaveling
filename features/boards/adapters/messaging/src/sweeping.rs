use std::sync::Arc;

use boards_core::{
    BoardCatalog, BoardError, BoardService, BoardServiceError, CatalogError, ProjectLink,
};
use eventpublishing::{UnreadableMessage, published_in};
use eventsourcing::{Agent, ServiceError};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use projects_contract::{DELETED, ProjectEventDTO};
use thiserror::Error;

const NAME: &str = "discard-boards-of-deleted-project";

pub struct DiscardBoardsOnProjectDeleted {
    boards: BoardService,
    catalog: Arc<dyn BoardCatalog>,
}

#[derive(Debug, Error)]
enum NotSwept {
    #[error(transparent)]
    Unreadable(#[from] UnreadableMessage),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Refused(#[from] BoardServiceError),
}

pub fn when_project_deleted() -> Subscription {
    Subscription::parse(DELETED).expect("a declared routing key holds no wildcards")
}

impl DiscardBoardsOnProjectDeleted {
    pub fn new(boards: BoardService, catalog: Arc<dyn BoardCatalog>) -> Self {
        Self { boards, catalog }
    }

    async fn discard_everything(&self, project: &ProjectLink) -> Result<(), NotSwept> {
        for summary in self.catalog.in_project(project).await? {
            match self
                .boards
                .discard(&summary.id.to_string(), None, &nobody())
                .await
            {
                Ok(_) => {}
                Err(refused) if already_discarded(&refused) => {}
                Err(refused) => return Err(refused.into()),
            }
        }

        Ok(())
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

    fn delivery(&self) -> Delivery {
        Delivery::Kept
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        self.work_through(message)
            .await
            .map_err(|why| NotHandled::because(self.named(), message.routing.clone(), why))
    }
}
