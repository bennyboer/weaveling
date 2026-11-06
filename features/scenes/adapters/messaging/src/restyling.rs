use std::sync::Arc;

use clock::Clock;
use messaging::{Failure, Handler, Message, MessageId, Publisher, WithoutInbox};
use projects_contract::{ProjectEventDTO, QUOTATION_CHANGED};
use quotation::QuotationMarks;
use scenes_contract::{MORE_QUOTATION_MARKS_TO_RESTYLE, MoreQuotationMarksToRestyleDTO};
use scenes_core::{QuotationRestyling, SceneId, SceneRestyler, ScenesCore};
use sha2::{Digest, Sha256};
use uuid::Builder;

use crate::AT_MOST;

pub struct RestyleOnQuotationChangedListener {
    scenes: ScenesCore,
    restyler: Arc<dyn SceneRestyler>,
    publisher: Arc<dyn Publisher>,
    clock: Arc<dyn Clock>,
    at_most: usize,
}

impl RestyleOnQuotationChangedListener {
    pub fn new(
        scenes: ScenesCore,
        restyler: Arc<dyn SceneRestyler>,
        publisher: Arc<dyn Publisher>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            scenes,
            restyler,
            publisher,
            clock,
            at_most: AT_MOST,
        }
    }

    pub fn taking_at_most(mut self, at_most: usize) -> Self {
        self.at_most = at_most;
        self
    }

    async fn restyle(&self, asked: &Message) -> Result<(), Failure> {
        let restyling = what_to_restyle(asked)?;
        let Some(last) = self
            .scenes
            .restyle(&restyling, self.at_most, self.restyler.as_ref())
            .await?
        else {
            return Ok(());
        };

        self.publisher
            .publish(self.carry_on(asked, &restyling, last))
            .await?;

        Ok(())
    }

    fn carry_on(&self, asked: &Message, task: &QuotationRestyling, after: SceneId) -> Message {
        Message {
            id: continuing(asked),
            ..asked.follow_up(
                MORE_QUOTATION_MARKS_TO_RESTYLE,
                &MoreQuotationMarksToRestyleDTO {
                    project: task.project.clone(),
                    quotation: task.style.name().to_owned(),
                    change: task.change,
                    after: after.to_string(),
                },
                self.clock.now(),
            )
        }
    }
}

fn continuing(asked: &Message) -> MessageId {
    let digest = Sha256::new()
        .chain_update(b"carry-on-restyling:")
        .chain_update(asked.id.as_uuid().as_bytes())
        .finalize();
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest[..16]);

    MessageId::of(Builder::from_custom_bytes(bytes).into_uuid())
}

fn what_to_restyle(asked: &Message) -> Result<QuotationRestyling, Failure> {
    if asked.routing.to_string() == MORE_QUOTATION_MARKS_TO_RESTYLE {
        let carried: MoreQuotationMarksToRestyleDTO = asked.payload_as()?;

        return Ok(QuotationRestyling {
            project: carried.project,
            style: QuotationMarks::new(&carried.quotation)?,
            change: carried.change,
            after: Some(carried.after),
        });
    }

    let changed = eventpublishing::published_in::<ProjectEventDTO>(asked)?;
    let ProjectEventDTO::QuotationChanged { quotation } = changed.event.body else {
        return Err("a project event that names no quotation style".into());
    };

    Ok(QuotationRestyling {
        project: changed.aggregate.id,
        style: QuotationMarks::new(&quotation)?,
        change: changed.aggregate.version,
        after: None,
    })
}

#[async_trait::async_trait]
impl Handler for RestyleOnQuotationChangedListener {
    const NAME: &'static str = "restyle-quotation-marks-of-project";
    const LISTENS_TO: &'static [&'static str] =
        &[QUOTATION_CHANGED, MORE_QUOTATION_MARKS_TO_RESTYLE];
    const WHEN_REFUSED: &'static str =
        "The quotation marks of a book were not all changed to its new style.";
    const WITHOUT_INBOX: WithoutInbox = WithoutInbox::Sweep;

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        self.restyle(message).await
    }
}
