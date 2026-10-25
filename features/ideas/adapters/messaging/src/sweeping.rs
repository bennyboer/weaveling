use std::sync::Arc;

use clock::Clock;
use ideas_contract::{MORE_TO_SWEEP, MoreToSweepDTO};
use ideas_core::{IdeaId, IdeasCore};
use messaging::{Failure, Handler, Message, Publisher, RoutingKey};
use projects_contract::{DELETED, ProjectEventDTO};

pub const AT_MOST: usize = 100;

pub struct DiscardOnProjectDeletedListener {
    ideas: IdeasCore,
    publisher: Arc<dyn Publisher>,
    clock: Arc<dyn Clock>,
    at_most: usize,
}

impl DiscardOnProjectDeletedListener {
    pub fn new(ideas: IdeasCore, publisher: Arc<dyn Publisher>, clock: Arc<dyn Clock>) -> Self {
        Self {
            ideas,
            publisher,
            clock,
            at_most: AT_MOST,
        }
    }

    pub fn taking_at_most(mut self, at_most: usize) -> Self {
        self.at_most = at_most;
        self
    }

    async fn sweep(&self, asked: &Message) -> Result<(), Failure> {
        let (project, after) = what_to_sweep(asked)?;
        let batch = self
            .ideas
            .sweep(&project, after.as_deref(), self.at_most)
            .await?;

        let Some(last) = batch.last().copied() else {
            return Ok(());
        };
        if batch.len() < self.at_most {
            return Ok(());
        }

        self.publisher
            .publish(self.carry_on(asked, &project, last))
            .await?;

        Ok(())
    }

    fn carry_on(&self, asked: &Message, project: &str, after: IdeaId) -> Message {
        asked.answering(
            RoutingKey::parse(MORE_TO_SWEEP).expect("a declared routing key holds no wildcards"),
            serde_json::to_value(MoreToSweepDTO {
                project: project.to_owned(),
                after: after.to_string(),
            })
            .expect("a continuation is plain data and cannot fail to serialize"),
            self.clock.now(),
        )
    }
}

fn what_to_sweep(asked: &Message) -> Result<(String, Option<String>), Failure> {
    if asked.routing.to_string() == MORE_TO_SWEEP {
        let carried: MoreToSweepDTO = asked.payload_as()?;

        return Ok((carried.project, Some(carried.after)));
    }

    let deleted = eventpublishing::published_in::<ProjectEventDTO>(asked)?;

    Ok((deleted.aggregate.id, None))
}

#[async_trait::async_trait]
impl Handler for DiscardOnProjectDeletedListener {
    const NAME: &'static str = "discard-ideas-of-deleted-project";
    const LISTENS_TO: &'static [&'static str] = &[DELETED, MORE_TO_SWEEP];
    const WHEN_REFUSED: &'static str = "The ideas of a deleted project were not cleared away.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        self.sweep(message).await
    }
}
