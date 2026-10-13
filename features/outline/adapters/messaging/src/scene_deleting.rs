use std::sync::Arc;

use messaging::{Delivery, Listener, ListenerName, Message, NotHandled, Subscription};
use outline_core::{Attachment, OutlineCatalog, OutlineService};
use scenes_contract::{DELETED, SceneDeletedDTO};
use thiserror::Error;

use crate::letting_go::{NotLetGo, let_go_everywhere};

const NAME: &str = "detach-deleted-scene";

pub struct DetachOnSceneDeleted {
    outlines: OutlineService,
    catalog: Arc<dyn OutlineCatalog>,
}

#[derive(Debug, Error)]
enum NotDetached {
    #[error("this message is not shaped like a deleted scene: {0}")]
    Unshaped(#[from] serde_json::Error),
    #[error(transparent)]
    NotLetGo(#[from] NotLetGo),
}

pub fn when_scene_deleted() -> Subscription {
    Subscription::parse(DELETED).expect("a declared routing key holds no wildcards")
}

impl DetachOnSceneDeleted {
    pub fn new(outlines: OutlineService, catalog: Arc<dyn OutlineCatalog>) -> Self {
        Self { outlines, catalog }
    }

    async fn work_through(&self, message: &Message) -> Result<(), NotDetached> {
        let deleted: SceneDeletedDTO = serde_json::from_value(message.payload.clone())?;
        let_go_everywhere(
            &self.outlines,
            self.catalog.as_ref(),
            &Attachment::scene(&deleted.scene),
        )
        .await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl Listener for DetachOnSceneDeleted {
    fn named(&self) -> ListenerName {
        ListenerName::parse(NAME).expect("the scene deletion listener is named at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        vec![when_scene_deleted()]
    }

    fn when_refused(&self) -> &'static str {
        "A deleted scene could not be taken out of the outline."
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
