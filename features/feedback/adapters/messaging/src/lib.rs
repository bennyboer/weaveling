use feedback_core::{FeedbackCore, KnownReadingCopy, ProjectLink, ReadingCopyLink};
use inbox::KeptHandler;
use messaging::{Failure, Message};
use projects_contract::context::PROJECT;
use readings_contract::{DELETED, EXTENDED, MADE, ReadingCopyEventDTO};
use transacting::Transaction;

/// Hears each reading copy made, extended and deleted, so feedback knows the project a copy was made of and until when it is open.
pub struct RememberReadingCopiesListener {
    feedback: FeedbackCore,
}

impl RememberReadingCopiesListener {
    pub fn new(feedback: FeedbackCore) -> Self {
        Self { feedback }
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
                return Ok(self
                    .feedback
                    .reading_copy_deleted(Some(transaction), &copy, message.occurred_at)
                    .await?);
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
