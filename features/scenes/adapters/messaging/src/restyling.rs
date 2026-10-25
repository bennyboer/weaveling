use std::sync::Arc;

use clock::Clock;
use messaging::{Failure, Handler, Message, Publisher, RoutingKey};
use projects_contract::{ProjectEventDTO, QUOTATION_CHANGED};
use quotation::QuotationMarks;
use scenes_contract::{MORE_QUOTATION_MARKS_TO_RESTYLE, MoreQuotationMarksToRestyleDTO};
use scenes_core::{QuotationRestyling, SceneId, SceneRestyler, ScenesCore};

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
        asked.answering(
            RoutingKey::parse(MORE_QUOTATION_MARKS_TO_RESTYLE)
                .expect("a declared routing key holds no wildcards"),
            serde_json::to_value(MoreQuotationMarksToRestyleDTO {
                project: task.project.clone(),
                quotation: task.style.name().to_owned(),
                change: task.change,
                after: after.to_string(),
            })
            .expect("a continuation is plain data and cannot fail to serialize"),
            self.clock.now(),
        )
    }
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

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        self.restyle(message).await
    }
}
