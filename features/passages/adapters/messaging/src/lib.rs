use eventpublishing::{UnreadableMessage, published_in};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use passages_core::{PassageService, PassageServiceError, StoreError};
use pieces_contract::{DISCARDED, PieceEventDTO};
use thiserror::Error;

const NAME: &str = "delete-passage-of-discarded-piece";

pub struct DeleteOnDiscard {
    passages: PassageService,
}

#[derive(Debug, Error)]
enum NotDeleted {
    #[error(transparent)]
    Unreadable(#[from] UnreadableMessage),
    #[error(transparent)]
    Passages(#[from] PassageServiceError),
}

pub fn when_discarded() -> Subscription {
    Subscription::parse(DISCARDED).expect("a declared routing key holds no wildcards")
}

impl DeleteOnDiscard {
    pub fn new(passages: PassageService) -> Self {
        Self { passages }
    }

    async fn work_through(&self, message: &Message) -> Result<(), NotDeleted> {
        let discarded = published_in::<PieceEventDTO>(message)?;

        let PieceEventDTO::Discarded {
            passage: Some(passage),
        } = discarded.event.body
        else {
            return Ok(());
        };

        match self.passages.delete(&passage).await {
            Ok(()) => Ok(()),
            Err(refused) if already_deleted(&refused) => Ok(()),
            Err(refused) => Err(refused.into()),
        }
    }
}

fn already_deleted(refused: &PassageServiceError) -> bool {
    matches!(refused, PassageServiceError::Store(StoreError::NotFound(_)))
}

#[async_trait::async_trait]
impl Listener for DeleteOnDiscard {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the discard listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![when_discarded()]
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
