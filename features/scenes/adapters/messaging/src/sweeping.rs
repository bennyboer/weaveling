use std::sync::Arc;

use clock::Clock;
use eventpublishing::{UnreadableMessage, published_in};
use messaging::{
    Delivery, Listener, ListenerName, Message, NotHandled, Publisher, RoutingKey, Subscription,
    Undelivered,
};
use projects_contract::{DELETED, ProjectEventDTO};
use scenes_contract::{MORE_TO_SWEEP, MoreToSweepDTO};
use scenes_core::{SceneId, SceneService, SceneServiceError, StoreError};
use thiserror::Error;

const NAME: &str = "delete-scenes-of-deleted-project";

pub const AT_MOST: usize = 100;

pub struct DeleteOnProjectDeleted {
    scenes: SceneService,
    publisher: Arc<dyn Publisher>,
    clock: Arc<dyn Clock>,
    at_most: usize,
}

#[derive(Debug, Error)]
enum NotSwept {
    #[error(transparent)]
    Unreadable(#[from] UnreadableMessage),
    #[error("this message does not say which project to sweep: {0}")]
    Unshaped(String),
    #[error(transparent)]
    Scenes(#[from] SceneServiceError),
    #[error(transparent)]
    Undelivered(#[from] Undelivered),
}

pub fn when_project_deleted() -> Subscription {
    Subscription::parse(DELETED).expect("a declared routing key holds no wildcards")
}

pub fn when_more_to_sweep() -> Subscription {
    Subscription::parse(MORE_TO_SWEEP).expect("a declared routing key holds no wildcards")
}

impl DeleteOnProjectDeleted {
    pub fn new(scenes: SceneService, publisher: Arc<dyn Publisher>, clock: Arc<dyn Clock>) -> Self {
        Self {
            scenes,
            publisher,
            clock,
            at_most: AT_MOST,
        }
    }

    pub fn taking_at_most(mut self, at_most: usize) -> Self {
        self.at_most = at_most;
        self
    }

    async fn sweep(&self, asked: &Message) -> Result<(), NotSwept> {
        let (project, after) = what_to_sweep(asked)?;
        let batch = self
            .scenes
            .in_project(&project, after.as_deref(), self.at_most)
            .await?;

        let Some(last) = batch.last().copied() else {
            return Ok(());
        };

        for scene in &batch {
            match self.scenes.delete(&scene.to_string()).await {
                Ok(()) => {}
                Err(refused) if already_deleted(&refused) => {}
                Err(refused) => return Err(refused.into()),
            }
        }

        if batch.len() < self.at_most {
            return Ok(());
        }

        self.publisher
            .publish(self.carry_on(asked, &project, last))
            .await?;

        Ok(())
    }

    fn carry_on(&self, asked: &Message, project: &str, after: SceneId) -> Message {
        asked.answering(
            RoutingKey::parse(MORE_TO_SWEEP).expect("a declared routing key holds no wildcards"),
            serde_json::to_value(MoreToSweepDTO {
                project: project.to_owned(),
                after: after.to_string(),
            })
            .expect("a continuation is plain data and cannot fail to serialize"),
            self.clock.now(),
        )
    }
}

fn what_to_sweep(asked: &Message) -> Result<(String, Option<String>), NotSwept> {
    if asked.routing.to_string() == MORE_TO_SWEEP {
        let carried: MoreToSweepDTO = serde_json::from_value(asked.payload.clone())
            .map_err(|why| NotSwept::Unshaped(why.to_string()))?;

        return Ok((carried.project, Some(carried.after)));
    }

    let deleted = published_in::<ProjectEventDTO>(asked)?;

    Ok((deleted.aggregate.id, None))
}

fn already_deleted(refused: &SceneServiceError) -> bool {
    matches!(refused, SceneServiceError::Store(StoreError::NotFound(_)))
}

#[async_trait::async_trait]
impl Listener for DeleteOnProjectDeleted {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the sweep listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![when_project_deleted(), when_more_to_sweep()]
    }

    fn when_refused(&self) -> &'static str {
        "The scenes of a deleted project were not cleared away."
    }

    fn delivery(&self) -> Delivery {
        Delivery::Kept
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        self.sweep(message)
            .await
            .map_err(|why| NotHandled::because(self.named(), message.routing.clone(), why))
    }
}
