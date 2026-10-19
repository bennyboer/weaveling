use eventpublishing::published_in;
use ideas_contract::{DISCARDED, IdeaEventDTO};
use messaging::{Failure, Handler, Message};
use scenes_core::SceneService;

pub struct UnlinkOnDiscardListener {
    scenes: SceneService,
}

impl UnlinkOnDiscardListener {
    pub fn new(scenes: SceneService) -> Self {
        Self { scenes }
    }
}

#[async_trait::async_trait]
impl Handler for UnlinkOnDiscardListener {
    const NAME: &'static str = "unlink-discarded-idea";
    const LISTENS_TO: &'static [&'static str] = &[DISCARDED];
    const WHEN_REFUSED: &'static str = "A discarded idea could not be unlinked from its scenes.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let discarded = published_in::<IdeaEventDTO>(message)?;

        Ok(self
            .scenes
            .unlink_everywhere(&discarded.aggregate.id)
            .await?)
    }
}
