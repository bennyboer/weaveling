use eventpublishing::{UnreadableMessage, published_in};
use ideas_contract::{DISCARDED, IdeaEventDTO};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use passages_core::{PassageService, PassageServiceError};
use thiserror::Error;

const NAME: &str = "unlink-discarded-idea";

pub struct UnlinkOnDiscard {
    passages: PassageService,
}

#[derive(Debug, Error)]
enum NotUnlinked {
    #[error(transparent)]
    Unreadable(#[from] UnreadableMessage),
    #[error(transparent)]
    Passages(#[from] PassageServiceError),
}

pub fn when_idea_discarded() -> Subscription {
    Subscription::parse(DISCARDED).expect("a declared routing key holds no wildcards")
}

impl UnlinkOnDiscard {
    pub fn new(passages: PassageService) -> Self {
        Self { passages }
    }

    async fn work_through(&self, message: &Message) -> Result<(), NotUnlinked> {
        let discarded = published_in::<IdeaEventDTO>(message)?;

        Ok(self
            .passages
            .unlink_everywhere(&discarded.aggregate.id)
            .await?)
    }
}

#[async_trait::async_trait]
impl Listener for UnlinkOnDiscard {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the unlink listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![when_idea_discarded()]
    }

    fn when_refused(&self) -> &'static str {
        "A discarded idea could not be unlinked from its passages."
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
