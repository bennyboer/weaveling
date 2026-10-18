use std::sync::Arc;

use clock::Clock;
use eventpublishing::{UnreadableMessage, published_in};
use messaging::{Failure, Handler, Message, Publisher, RoutingKey, Undelivered};
use projects_contract::{ProjectEventDTO, QUOTATION_CHANGED};
use quotation::{InvalidQuotationMarks, QuotationMarks};
use scenes_contract::{MORE_TO_RESTYLE, MoreToRestyleDTO};
use scenes_core::{
    ProjectLink, RestylingTaskError, RestylingTasks, SceneId, SceneRestyler, SceneService,
    SceneServiceError, StoreError,
};
use thiserror::Error;

use crate::AT_MOST;

pub struct RestyleOnQuotationChanged {
    scenes: SceneService,
    restyler: Arc<dyn SceneRestyler>,
    tasks: Arc<dyn RestylingTasks>,
    publisher: Arc<dyn Publisher>,
    clock: Arc<dyn Clock>,
    at_most: usize,
}

#[derive(Debug, Error)]
enum NotRestyled {
    #[error(transparent)]
    Unreadable(#[from] UnreadableMessage),
    #[error("this message does not say which project to restyle: {0}")]
    Unshaped(String),
    #[error(transparent)]
    Unstyled(#[from] InvalidQuotationMarks),
    #[error(transparent)]
    RestylingTasks(#[from] RestylingTaskError),
    #[error(transparent)]
    Scenes(#[from] SceneServiceError),
    #[error(transparent)]
    Undelivered(#[from] Undelivered),
}

struct RestylingTask {
    project: String,
    style: QuotationMarks,
    change: u64,
    after: Option<String>,
}

impl RestyleOnQuotationChanged {
    pub fn new(
        scenes: SceneService,
        restyler: Arc<dyn SceneRestyler>,
        tasks: Arc<dyn RestylingTasks>,
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

    async fn restyle(&self, asked: &Message) -> Result<(), NotRestyled> {
        let task = what_to_restyle(asked)?;
        if !self.still_wanted(&task).await? {
            return Ok(());
        }
        let batch = self
            .scenes
            .in_project(&task.project, task.after.as_deref(), self.at_most)
            .await?;

        let Some(last) = batch.last().copied() else {
            return Ok(());
        };

        for scene in &batch {
            match self.restyler.restyle(*scene, task.style).await {
                Ok(_) => {}
                Err(refused) if deleted_meanwhile(&refused) => {}
                Err(refused) => return Err(refused.into()),
            }
        }

        if batch.len() < self.at_most {
            return Ok(());
        }

        self.publisher
            .publish(self.carry_on(asked, &task, last))
            .await?;

        Ok(())
    }

    async fn still_wanted(&self, task: &RestylingTask) -> Result<bool, RestylingTaskError> {
        let project = ProjectLink::from(task.project.as_str());

        match task.after {
            None => self.tasks.begin(&project, task.change).await,
            Some(_) => self.tasks.is_latest(&project, task.change).await,
        }
    }

    fn carry_on(&self, asked: &Message, task: &RestylingTask, after: SceneId) -> Message {
        asked.answering(
            RoutingKey::parse(MORE_TO_RESTYLE).expect("a declared routing key holds no wildcards"),
            serde_json::to_value(MoreToRestyleDTO {
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

fn what_to_restyle(asked: &Message) -> Result<RestylingTask, NotRestyled> {
    if asked.routing.to_string() == MORE_TO_RESTYLE {
        let carried: MoreToRestyleDTO = serde_json::from_value(asked.payload.clone())
            .map_err(|why| NotRestyled::Unshaped(why.to_string()))?;

        return Ok(RestylingTask {
            project: carried.project,
            style: QuotationMarks::new(&carried.quotation)?,
            change: carried.change,
            after: None,
        });
    }

    let changed = published_in::<ProjectEventDTO>(asked)?;
    let ProjectEventDTO::QuotationChanged { quotation } = changed.event.body else {
        return Err(NotRestyled::Unshaped(
            "a project event that names no quotation style".to_owned(),
        ));
    };

    Ok(RestylingTask {
        project: changed.aggregate.id,
        style: QuotationMarks::new(&quotation)?,
        change: changed.aggregate.version,
        after: None,
    })
}

fn deleted_meanwhile(refused: &SceneServiceError) -> bool {
    matches!(refused, SceneServiceError::Store(StoreError::NotFound(_)))
}

#[async_trait::async_trait]
impl Handler for RestyleOnQuotationChanged {
    const NAME: &'static str = "restyle-scenes-of-restyled-project";
    const LISTENS_TO: &'static [&'static str] = &[QUOTATION_CHANGED, MORE_TO_RESTYLE];
    const WHEN_REFUSED: &'static str =
        "The quotation marks of a book were not all changed to its new style.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        Ok(self.restyle(message).await?)
    }
}
