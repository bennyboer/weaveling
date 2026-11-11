use async_trait::async_trait;
use ideas_contract::{CAPTURED, IdeaEventDTO};
use inbox::KeptHandler;
use messaging::{Failure, Message};
use outline_core::{Attachment, OutlinesCore};
use scenes_contract::{CREATED, SceneCreatedDTO};
use transacting::Transaction;

pub struct RememberProjectsOfAttachmentsListener {
    outlines: OutlinesCore,
}

impl RememberProjectsOfAttachmentsListener {
    pub fn new(outlines: OutlinesCore) -> Self {
        Self { outlines }
    }
}

#[async_trait]
impl KeptHandler for RememberProjectsOfAttachmentsListener {
    const NAME: &'static str = "outline-remember-projects-of-attachments";
    const LISTENS_TO: &'static [&'static str] = &[CAPTURED, CREATED];
    const WHEN_REFUSED: &'static str = "The project of an idea or a scene did not reach the outline, so an outline of another project could attach it.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let (attachment, project) = if message.routing.to_string() == CREATED {
            let created: SceneCreatedDTO = message.payload_as()?;
            (Attachment::scene(&created.scene), created.project)
        } else {
            let captured = eventpublishing::published_in::<IdeaEventDTO>(message)?;
            let IdeaEventDTO::Captured { project, .. } = captured.event.body else {
                return Ok(());
            };
            (Attachment::idea(&captured.aggregate.id), project)
        };

        Ok(self
            .outlines
            .came_to_be(Some(transaction), &attachment, &project)
            .await?)
    }
}
