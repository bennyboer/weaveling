use async_trait::async_trait;
use ideas_contract::EVERY_IDEA;
use ideas_core::{IdeasCore, IdeasError};
use live::Broadcaster;
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use serde_json::Value;

pub const LIVE: &str = "broadcast-idea-changes";

pub struct BroadcastIdeaChangesListener {
    ideas: IdeasCore,
    broadcaster: Broadcaster,
}

impl BroadcastIdeaChangesListener {
    pub fn new(ideas: IdeasCore, broadcaster: Broadcaster) -> Self {
        Self { ideas, broadcaster }
    }
}

#[async_trait]
impl Listener for BroadcastIdeaChangesListener {
    fn named(&self) -> ListenerName {
        ListenerName::parse(LIVE).expect("a broadcast's name is written at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![
            Subscription::parse(EVERY_IDEA)
                .expect("a broadcast's subscription is written at compile time"),
        ]
    }

    fn when_refused(&self) -> &'static str {
        "A change to an idea was not passed on to whoever has its project open."
    }

    fn delivery(&self) -> Delivery {
        Delivery::Fleeting
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        let Ok(published) = eventpublishing::published_in::<Value>(message) else {
            return Ok(());
        };
        let project = match self.ideas.project_of(&published.aggregate.id).await {
            Ok(project) => project,
            Err(IdeasError::InvalidId(_)) => return Ok(()),
            Err(because) => {
                return Err(NotHandled {
                    listener: self.named(),
                    routing: message.routing.clone(),
                    because: Box::new(because),
                });
            }
        };
        self.broadcaster
            .tell(project.as_str(), live::frame_of(&published));

        Ok(())
    }
}
