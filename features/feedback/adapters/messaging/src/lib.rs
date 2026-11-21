use async_trait::async_trait;
use feedback_core::{FeedbackCore, ReadingCopyLink};
use messaging::{Failure, Handler, Message, WithoutInbox};
use projects_contract::context::PROJECT;
use readings_contract::MADE;

/// Hears each reading copy made, and the project it was made of, so feedback on it is found in that project.
pub struct RememberCopiesListener {
    feedback: FeedbackCore,
}

impl RememberCopiesListener {
    pub fn new(feedback: FeedbackCore) -> Self {
        Self { feedback }
    }
}

#[async_trait]
impl Handler for RememberCopiesListener {
    const NAME: &'static str = "feedback-remember-copies";
    const LISTENS_TO: &'static [&'static str] = &[MADE];
    const WHEN_REFUSED: &'static str = "The feedback feature did not learn which project a reading copy was made of. Its feedback is not found in the project's feedback view, nor counted there.";
    // TODO: kept through the feature's own inbox once feedback is kept (step 5)
    const WITHOUT_INBOX: WithoutInbox = WithoutInbox::Fleeting;

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let copy: String = eventpublishing::aggregate_in(message)?;
        let context = eventpublishing::context_in(message);
        let project = context
            .get(PROJECT)
            .ok_or("a reading copy made tells no project")?;
        self.feedback
            .remember_copy(ReadingCopyLink::from(copy), project);

        Ok(())
    }
}
