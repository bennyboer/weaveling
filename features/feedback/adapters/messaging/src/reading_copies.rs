use std::sync::Arc;

use clock::Clock;
use feedback_core::{FeedbackCore, KnownReadingCopy, ProjectLink, ReadingCopyLink};
use inbox::KeptHandler;
use messaging::{Failure, Handler, Message, WithoutInbox};
use outbox::{Origin, Outbox};
use projects_contract::context::PROJECT;
use readings_contract::{DELETED, EXTENDED, MADE, ReadingCopyEventDTO};
use serde::{Deserialize, Serialize};
use transacting::Transaction;

pub const SWEEP_READING_COPY: &str = "follow-up.feedback.sweep-reading-copy";

/// A deleted reading copy whose feedback is still to be swept.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SweepReadingCopyDTO {
    pub reading_copy: String,
}

/// Hears each reading copy made, extended and deleted, so feedback knows the project a copy was made of and until when it is open.
/// Once a copy is deleted, it asks for its feedback to be swept, in the transaction that forgets the copy.
pub struct RememberReadingCopiesListener {
    feedback: FeedbackCore,
    outbox: Arc<dyn Outbox>,
    clock: Arc<dyn Clock>,
}

impl RememberReadingCopiesListener {
    pub fn new(feedback: FeedbackCore, outbox: Arc<dyn Outbox>, clock: Arc<dyn Clock>) -> Self {
        Self {
            feedback,
            outbox,
            clock,
        }
    }
}

#[async_trait::async_trait]
impl KeptHandler for RememberReadingCopiesListener {
    const NAME: &'static str = "feedback-remember-reading-copies";
    const LISTENS_TO: &'static [&'static str] = &[MADE, EXTENDED, DELETED];
    const WHEN_REFUSED: &'static str = "A reading copy made, extended or deleted did not reach the feedback feature. Its feedback may be missing from its project's feedback, may still be given after the copy closed, or refused while it is open.";

    /// Keeps the copy as the newest message tells it, or forgets it once it is deleted.
    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<ReadingCopyEventDTO>(message)?;
        let copy = ReadingCopyLink::from(told.aggregate.id);
        let open_until = match told.event.body {
            ReadingCopyEventDTO::Made { open_until, .. }
            | ReadingCopyEventDTO::Extended { open_until, .. } => open_until,
            ReadingCopyEventDTO::Deleted => {
                self.feedback
                    .reading_copy_deleted(Some(transaction), &copy, message.occurred_at)
                    .await?;
                self.outbox
                    .enqueue_follow_up_within(
                        transaction,
                        Origin {
                            aggregate: copy.as_str(),
                            kind: "reading-copy",
                            version: Some(told.aggregate.version),
                        },
                        message,
                        SWEEP_READING_COPY,
                        &SweepReadingCopyDTO {
                            reading_copy: copy.to_string(),
                        },
                        self.clock.now(),
                    )
                    .await?;

                return Ok(());
            }
            ReadingCopyEventDTO::Renamed { .. } => {
                return Err("a copy renamed tells feedback nothing".into());
            }
        };
        let project = told
            .context
            .get(PROJECT)
            .ok_or("a reading copy's message tells no project")?;
        let open_until = match open_until {
            Some(until) => {
                Some(clock::text::read(&until).ok_or("a reading copy is open until no time")?)
            }
            None => None,
        };

        Ok(self
            .feedback
            .reading_copy_heard(
                Some(transaction),
                &copy,
                &KnownReadingCopy {
                    project: ProjectLink::from(project.as_str()),
                    open_until,
                },
                told.aggregate.version,
            )
            .await?)
    }
}

/// Deletes the feedback of a deleted reading copy, drafts and all, erasing every word of it.
pub struct SweepFeedbackOfDeletedReadingCopyListener {
    feedback: FeedbackCore,
}

impl SweepFeedbackOfDeletedReadingCopyListener {
    pub fn new(feedback: FeedbackCore) -> Self {
        Self { feedback }
    }
}

#[async_trait::async_trait]
impl Handler for SweepFeedbackOfDeletedReadingCopyListener {
    const NAME: &'static str = "feedback-sweep-deleted-reading-copy";
    const LISTENS_TO: &'static [&'static str] = &[SWEEP_READING_COPY];
    const WHEN_REFUSED: &'static str = "The feedback of a deleted reading copy was not deleted. Its words stay stored, though nobody can find them any more.";
    const WITHOUT_INBOX: WithoutInbox = WithoutInbox::Sweep;

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let asked: SweepReadingCopyDTO = message.payload_as()?;

        Ok(self
            .feedback
            .sweep_reading_copy(&ReadingCopyLink::from(asked.reading_copy))
            .await?)
    }
}
