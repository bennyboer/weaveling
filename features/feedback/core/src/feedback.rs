use std::sync::{Arc, Mutex, MutexGuard};

use clock::Clock;
use eventsourcing::Agent;
use time::OffsetDateTime;

use crate::about::{About, Subject};
use crate::comment::{Comment, CommentBlock};
use crate::error::FeedbackError;
use crate::id::FeedbackId;

/// A piece of feedback, as whoever may see it finds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feedback {
    pub id: FeedbackId,
    pub about: About,
    pub comment: Comment,
    /// Whether it is still a draft, which only whoever wrote it sees.
    pub draft: bool,
    pub written_by: Agent,
    pub written_at: OffsetDateTime,
}

/// The feedback feature's only way in.
#[derive(Clone)]
pub struct FeedbackCore {
    // TODO: the `Feedback` aggregate in an event store and its catalogs, rather than what this process holds (step 5)
    kept: Arc<Mutex<Vec<Feedback>>>,
    clock: Arc<dyn Clock>,
}

impl FeedbackCore {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            kept: Arc::new(Mutex::new(Vec::new())),
            clock,
        }
    }

    fn kept(&self) -> MutexGuard<'_, Vec<Feedback>> {
        self.kept.lock().expect("feedback lock poisoned")
    }

    /// The feedback given on a subject that the agent may see, oldest first: for now their own drafts.
    pub async fn feedback_on(
        &self,
        subject: &Subject,
        agent: &Agent,
    ) -> Result<Vec<Feedback>, FeedbackError> {
        // TODO: only for whoever may read the subject's project, and the feedback everyone submitted (step 2)
        Ok(self
            .kept()
            .iter()
            .filter(|feedback| {
                &feedback.about.subject() == subject && &feedback.written_by == agent
            })
            .cloned()
            .collect())
    }

    /// Starts a draft about something, which only whoever writes it sees until they submit it.
    pub async fn draft(
        &self,
        about: About,
        comment: Vec<CommentBlock>,
        agent: &Agent,
    ) -> Result<Feedback, FeedbackError> {
        // TODO: only on a subject that is open, for whoever may read its project (step 5)
        let comment = Comment::new(comment)?;
        if !about.holds_something() {
            return Err(FeedbackError::PassageEmpty);
        }
        let now = self.clock.now();
        let drafted = Feedback {
            id: FeedbackId::generate(now),
            about,
            comment,
            draft: true,
            written_by: agent.clone(),
            written_at: now,
        };
        self.kept().push(drafted.clone());

        Ok(drafted)
    }

    /// Rewrites what a piece of feedback says, for whoever wrote it.
    pub async fn rewrite(
        &self,
        feedback: &str,
        comment: Vec<CommentBlock>,
        agent: &Agent,
    ) -> Result<Feedback, FeedbackError> {
        let comment = Comment::new(comment)?;
        let mut kept = self.kept();
        let rewritten = written_by(&mut kept, feedback, agent)?;
        rewritten.comment = comment;

        Ok(rewritten.clone())
    }

    /// Deletes a piece of feedback, for whoever wrote it.
    pub async fn delete(&self, feedback: &str, agent: &Agent) -> Result<(), FeedbackError> {
        // TODO: an owner of the copy's project deletes any feedback too (step 5)
        let mut kept = self.kept();
        let id = written_by(&mut kept, feedback, agent)?.id;
        kept.retain(|feedback| feedback.id != id);

        Ok(())
    }
}

/// The feedback, if the agent wrote it.
/// Someone else's draft is not there for them at all, and someone else's submitted feedback is not theirs to change.
fn written_by<'a>(
    kept: &'a mut [Feedback],
    feedback: &str,
    agent: &Agent,
) -> Result<&'a mut Feedback, FeedbackError> {
    let found = kept
        .iter_mut()
        .find(|kept| kept.id.to_string() == feedback)
        .ok_or(FeedbackError::FeedbackNotThere)?;
    if &found.written_by != agent {
        return Err(if found.draft {
            FeedbackError::FeedbackNotThere
        } else {
            FeedbackError::NotYours
        });
    }

    Ok(found)
}
