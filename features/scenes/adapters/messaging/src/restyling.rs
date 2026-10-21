use std::sync::Arc;

use clock::Clock;
use eventpublishing::published_in;
use messaging::{Failure, Handler, Message, Publisher, RoutingKey};
use projects_contract::{ProjectEventDTO, QUOTATION_CHANGED};
use quotation::QuotationMarks;
use scenes_contract::{MORE_QUOTATION_MARKS_TO_RESTYLE, MoreQuotationMarksToRestyleDTO};
use scenes_core::{
    ProjectLink, QuotationRestylingTasks, SceneId, SceneRestyler, SceneService, SceneServiceError,
    StoreError,
};

use crate::AT_MOST;

pub struct RestyleOnQuotationChangedListener {
    scenes: SceneService,
    restyler: Arc<dyn SceneRestyler>,
    tasks: Arc<dyn QuotationRestylingTasks>,
    publisher: Arc<dyn Publisher>,
    clock: Arc<dyn Clock>,
    at_most: usize,
}

struct QuotationRestylingTask {
    project: String,
    style: QuotationMarks,
    change: u64,
    after: Option<String>,
}

impl RestyleOnQuotationChangedListener {
    pub fn new(
        scenes: SceneService,
        restyler: Arc<dyn SceneRestyler>,
        tasks: Arc<dyn QuotationRestylingTasks>,
        publisher: Arc<dyn Publisher>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            scenes,
            restyler,
            tasks,
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
        let task = what_to_restyle(asked)?;
        let project = ProjectLink::from(task.project.as_str());
        if !self.claimed(&project, &task).await? {
            return Ok(());
        }
        let batch = self
            .scenes
            .in_project(&task.project, task.after.as_deref(), self.at_most)
            .await?;

        for scene in &batch {
            match self.restyler.restyle(*scene, task.style).await {
                Ok(_) => {}
                Err(refused) if deleted_meanwhile(&refused) => {}
                Err(refused) => return Err(refused.into()),
            }
        }

        let finished = batch.len() < self.at_most;
        self.tasks
            .progressed(&project, task.change, in_count(batch.len()), finished)
            .await?;
        let Some(last) = batch.last().copied() else {
            return Ok(());
        };
        if finished {
            return Ok(());
        }

        self.publisher
            .publish(self.carry_on(asked, &task, last))
            .await?;

        Ok(())
    }

    async fn claimed(
        &self,
        project: &ProjectLink,
        task: &QuotationRestylingTask,
    ) -> Result<bool, Failure> {
        if task.after.is_some() {
            return Ok(self.tasks.claim(project, task.change).await?);
        }
        let total = self.scenes_in(&task.project).await?;

        Ok(self.tasks.begin(project, task.change, total).await?)
    }

    async fn scenes_in(&self, project: &str) -> Result<u64, SceneServiceError> {
        let mut total = 0;
        let mut after: Option<String> = None;

        loop {
            let page = self
                .scenes
                .in_project(project, after.as_deref(), self.at_most)
                .await?;
            total += in_count(page.len());
            match page.last() {
                Some(last) if page.len() == self.at_most => after = Some(last.to_string()),
                _ => return Ok(total),
            }
        }
    }

    fn carry_on(&self, asked: &Message, task: &QuotationRestylingTask, after: SceneId) -> Message {
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

fn what_to_restyle(asked: &Message) -> Result<QuotationRestylingTask, Failure> {
    if asked.routing.to_string() == MORE_QUOTATION_MARKS_TO_RESTYLE {
        let carried: MoreQuotationMarksToRestyleDTO = asked.payload_as()?;

        return Ok(QuotationRestylingTask {
            project: carried.project,
            style: QuotationMarks::new(&carried.quotation)?,
            change: carried.change,
            after: Some(carried.after),
        });
    }

    let changed = published_in::<ProjectEventDTO>(asked)?;
    let ProjectEventDTO::QuotationChanged { quotation } = changed.event.body else {
        return Err("a project event that names no quotation style".into());
    };

    Ok(QuotationRestylingTask {
        project: changed.aggregate.id,
        style: QuotationMarks::new(&quotation)?,
        change: changed.aggregate.version,
        after: None,
    })
}

fn in_count(scenes: usize) -> u64 {
    u64::try_from(scenes).expect("a number of scenes fits 64 bits")
}

fn deleted_meanwhile(refused: &SceneServiceError) -> bool {
    matches!(refused, SceneServiceError::Store(StoreError::NotFound(_)))
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
