use std::sync::Arc;

use clock::Clock;
use ideas_contract::{MORE_TO_SWEEP, MoreToSweepDTO};
use ideas_core::IdeasCore;
use messaging::{Failure, Handler, Message, Publisher, WithoutInbox};
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
        if after.is_none() {
            self.ideas
                .lay_project_tombstone(&project, asked.occurred_at)
                .await?;
        }
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
            .publish(asked.follow_up(
                MORE_TO_SWEEP,
                &MoreToSweepDTO {
                    project: project.clone(),
                    after: last.to_string(),
                },
                self.clock.now(),
            ))
            .await?;

        Ok(())
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
    const WHEN_REFUSED: &'static str = "The ideas of a deleted project were not removed.";
    const WITHOUT_INBOX: WithoutInbox = WithoutInbox::Sweep;

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        self.sweep(message).await
    }
}
