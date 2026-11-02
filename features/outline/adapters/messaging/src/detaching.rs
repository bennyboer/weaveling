use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use ideas_contract::{DISCARDED, IdeaEventDTO};
use inbox::KeptHandler;
use messaging::{Failure, Message, RoutingKey};
use outbox::{Origin, Outbox};
use outline_contract::AttachmentDTO;
use outline_core::{Attachment, OutlinesCore};
use scenes_contract::{DELETED, SceneDeletedDTO};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use transacting::Transaction;

use crate::publishing;

pub const DETACH_EVERYWHERE: &str = "follow-up.outline.detach-everywhere";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct DetachEverywhereDTO {
    pub attachment: AttachmentDTO,
}

pub struct DetachOnDiscardListener {
    outlines: OutlinesCore,
    outbox: Arc<dyn Outbox>,
    clock: Arc<dyn Clock>,
}

impl DetachOnDiscardListener {
    pub fn new(outlines: OutlinesCore, outbox: Arc<dyn Outbox>, clock: Arc<dyn Clock>) -> Self {
        Self {
            outlines,
            outbox,
            clock,
        }
    }
}

#[async_trait]
impl KeptHandler for DetachOnDiscardListener {
    const NAME: &'static str = "detach-discarded-idea";
    const LISTENS_TO: &'static [&'static str] = &[DISCARDED];
    const WHEN_REFUSED: &'static str = "A discarded idea could not be taken out of the outline.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let discarded = eventpublishing::published_in::<IdeaEventDTO>(message)?;
        let gone = Attachment::idea(&discarded.aggregate.id);

        self.outlines
            .lay_tombstone(Some(&mut *transaction), &gone, message.occurred_at)
            .await?;
        self.outbox
            .enqueue_within(
                transaction,
                Origin::unversioned(gone.as_str(), "idea"),
                &detach_everywhere(message, &gone, self.clock.now()),
            )
            .await?;

        Ok(())
    }
}

pub struct DetachOnSceneDeletedListener {
    outlines: OutlinesCore,
    outbox: Arc<dyn Outbox>,
    clock: Arc<dyn Clock>,
}

impl DetachOnSceneDeletedListener {
    pub fn new(outlines: OutlinesCore, outbox: Arc<dyn Outbox>, clock: Arc<dyn Clock>) -> Self {
        Self {
            outlines,
            outbox,
            clock,
        }
    }
}

#[async_trait]
impl KeptHandler for DetachOnSceneDeletedListener {
    const NAME: &'static str = "detach-deleted-scene";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str = "A deleted scene could not be taken out of the outline.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let deleted: SceneDeletedDTO = message.payload_as()?;
        let gone = Attachment::scene(&deleted.scene);

        self.outlines
            .lay_tombstone(Some(&mut *transaction), &gone, message.occurred_at)
            .await?;
        self.outbox
            .enqueue_within(
                transaction,
                Origin::unversioned(gone.as_str(), "scene"),
                &detach_everywhere(message, &gone, self.clock.now()),
            )
            .await?;

        Ok(())
    }
}

fn detach_everywhere(heard: &Message, gone: &Attachment, now: OffsetDateTime) -> Message {
    heard.answering(
        RoutingKey::parse(DETACH_EVERYWHERE).expect("a follow-up's routing key holds no wildcards"),
        json!(DetachEverywhereDTO {
            attachment: publishing::to_dto(gone),
        }),
        now,
    )
}

pub struct DetachEverywhereListener {
    outlines: OutlinesCore,
}

impl DetachEverywhereListener {
    pub fn new(outlines: OutlinesCore) -> Self {
        Self { outlines }
    }
}

#[async_trait]
impl KeptHandler for DetachEverywhereListener {
    const NAME: &'static str = "detach-everywhere";
    const LISTENS_TO: &'static [&'static str] = &[DETACH_EVERYWHERE];
    const WHEN_REFUSED: &'static str =
        "A discarded idea or a deleted scene could not be taken out of the outline.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let asked: DetachEverywhereDTO = message.payload_as()?;

        Ok(self
            .outlines
            .let_go_everywhere(Some(transaction), &of_dto(&asked.attachment))
            .await?)
    }
}

fn of_dto(attachment: &AttachmentDTO) -> Attachment {
    match attachment {
        AttachmentDTO::Scene { id } => Attachment::scene(id),
        AttachmentDTO::Idea { id } => Attachment::idea(id),
    }
}
