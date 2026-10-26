use messaging::{Failure, Handler, Message};
use outline_core::{Attachment, OutlinesCore};
use scenes_contract::{DELETED, SceneDeletedDTO};

pub struct DetachOnSceneDeletedListener {
    outlines: OutlinesCore,
}

impl DetachOnSceneDeletedListener {
    pub fn new(outlines: OutlinesCore) -> Self {
        Self { outlines }
    }
}

#[async_trait::async_trait]
impl Handler for DetachOnSceneDeletedListener {
    const NAME: &'static str = "detach-deleted-scene";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str = "A deleted scene could not be taken out of the outline.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let deleted: SceneDeletedDTO = message.payload_as()?;

        Ok(self
            .outlines
            .let_go_everywhere(&Attachment::scene(&deleted.scene), message.occurred_at)
            .await?)
    }
}
