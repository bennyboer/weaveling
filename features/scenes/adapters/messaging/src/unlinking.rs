use eventpublishing::{UnreadableMessage, published_in};
use ideas_contract::{DISCARDED, IdeaEventDTO};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use scenes_core::{SceneService, SceneServiceError};
use thiserror::Error;

const NAME: &str = "unlink-discarded-idea";

pub struct UnlinkOnDiscard {
    scenes: SceneService,
}

#[derive(Debug, Error)]
enum NotUnlinked {
    #[error(transparent)]
    Unreadable(#[from] UnreadableMessage),
    #[error(transparent)]
    Scenes(#[from] SceneServiceError),
}

pub fn when_idea_discarded() -> Subscription {
    Subscription::parse(DISCARDED).expect("a declared routing key holds no wildcards")
}

impl UnlinkOnDiscard {
    pub fn new(scenes: SceneService) -> Self {
        Self { scenes }
    }

    async fn work_through(&self, message: &Message) -> Result<(), NotUnlinked> {
        let discarded = published_in::<IdeaEventDTO>(message)?;

        Ok(self
            .scenes
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
        "A discarded idea could not be unlinked from its scenes."
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
