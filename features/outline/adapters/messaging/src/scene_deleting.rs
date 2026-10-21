use std::sync::Arc;

use messaging::{Failure, Handler, Message};
use outline_core::{Attachment, OutlineCatalog, OutlineService};
use scenes_contract::{DELETED, SceneDeletedDTO};

use crate::letting_go::let_go_everywhere;

pub struct DetachOnSceneDeletedListener {
    outlines: OutlineService,
    catalog: Arc<dyn OutlineCatalog>,
}

impl DetachOnSceneDeletedListener {
    pub fn new(outlines: OutlineService, catalog: Arc<dyn OutlineCatalog>) -> Self {
        Self { outlines, catalog }
    }

    async fn work_through(&self, message: &Message) -> Result<(), Failure> {
        let deleted: SceneDeletedDTO = message.payload_as()?;
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
impl Handler for DetachOnSceneDeletedListener {
    const NAME: &'static str = "detach-deleted-scene";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str = "A deleted scene could not be taken out of the outline.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        self.work_through(message).await
    }
}
