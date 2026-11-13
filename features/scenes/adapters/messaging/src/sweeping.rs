use std::sync::Arc;

use clock::Clock;
use messaging::{Failure, Handler, Message, Publisher, WithoutInbox};
use projects_contract::{DELETED, ProjectEventDTO};
use scenes_contract::{MORE_TO_SWEEP, MoreToSweepDTO};
use scenes_core::ScenesCore;

pub const AT_MOST: usize = 100;

pub struct DeleteOnProjectDeletedListener {
    scenes: ScenesCore,
    publisher: Arc<dyn Publisher>,
    clock: Arc<dyn Clock>,
    at_most: usize,
}

impl DeleteOnProjectDeletedListener {
    pub fn new(scenes: ScenesCore, publisher: Arc<dyn Publisher>, clock: Arc<dyn Clock>) -> Self {
        Self {
            scenes,
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
            self.scenes
                .lay_project_tombstone(None, &project, asked.occurred_at)
                .await?;
        }
        let batch = self
            .scenes
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
impl Handler for DeleteOnProjectDeletedListener {
    const NAME: &'static str = "delete-scenes-of-deleted-project";
    const LISTENS_TO: &'static [&'static str] = &[DELETED, MORE_TO_SWEEP];
    const WHEN_REFUSED: &'static str = "The scenes of a deleted project were not removed. New scenes may still be created in that project.";
    const WITHOUT_INBOX: WithoutInbox = WithoutInbox::Sweep;

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        self.sweep(message).await
    }
}
