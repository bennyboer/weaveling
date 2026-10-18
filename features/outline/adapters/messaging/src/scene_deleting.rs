use std::sync::Arc;

use messaging::{Failure, Handler, Message};
use outline_core::{Attachment, OutlineCatalog, OutlineService};
use scenes_contract::{DELETED, SceneDeletedDTO};
use thiserror::Error;

use crate::letting_go::{NotLetGo, let_go_everywhere};

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
impl Handler for DetachOnSceneDeleted {
    const NAME: &'static str = "detach-deleted-scene";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str = "A deleted scene could not be taken out of the outline.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        Ok(self.work_through(message).await?)
    }
}
