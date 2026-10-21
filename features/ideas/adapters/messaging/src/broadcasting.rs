use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use eventpublishing::published_in;
use eventsourcing::{AggregateId, EventSourcingService, EventStore};
use ideas_contract::EVERY_IDEA;
use ideas_core::{Idea, IdeaEvent, IdeaId};
use live::{Broadcaster, frame_of};
use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use serde_json::Value;

pub const LIVE: &str = "broadcast-idea-changes";

pub struct BroadcastIdeaChangesListener {
    events: EventSourcingService<Idea>,
    broadcaster: Broadcaster,
}

impl BroadcastIdeaChangesListener {
    pub fn new(
        store: Arc<dyn EventStore<IdeaEvent>>,
        broadcaster: Broadcaster,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: EventSourcingService::new(store, clock),
            broadcaster,
        }
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
        let Ok(published) = published_in::<Value>(message) else {
            return Ok(());
        };
        let Ok(id) = published.aggregate.id.parse::<IdeaId>() else {
            return Ok(());
        };
        let standing = self
            .events
            .latest(&AggregateId::from(&id))
            .await
            .map_err(|because| NotHandled {
                listener: self.named(),
                routing: message.routing.clone(),
                because: Box::new(because),
            })?;
        self.broadcaster
            .tell(standing.state.project().as_str(), frame_of(&published));

        Ok(())
    }
}
