use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use clock::Clock;
use eventsourcing::Agent;
use time::OffsetDateTime;

use crate::about::{About, Subject};
use crate::comment::{Comment, CommentBlock};
use crate::error::FeedbackError;
use crate::id::FeedbackId;
use crate::link::{SceneLink, SectionLink};

/// A piece of feedback, as whoever may see it finds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feedback {
    pub id: FeedbackId,
    pub about: About,
    pub comment: Comment,
    /// When it was submitted; until then it is a draft, which only whoever wrote it sees.
    pub submitted_at: Option<OffsetDateTime>,
    /// Whether what it says was changed after it was submitted.
    pub edited: bool,
    /// Whether the authors marked it done.
    pub done: bool,
    pub written_by: Agent,
    pub written_at: OffsetDateTime,
}

impl Feedback {
    pub fn is_draft(&self) -> bool {
        self.submitted_at.is_none()
    }
}

/// How much feedback a subject holds, as one who may see it counts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counted {
    pub subject: Subject,
    /// The feedback submitted and not done yet.
    pub not_done: usize,
    /// The asker's own drafts.
    pub your_drafts: usize,
}

/// How much feedback given is not done yet on each scene, on it or a passage in it, and on each section itself, in every copy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pending {
    pub scenes: BTreeMap<SceneLink, usize>,
    pub sections: BTreeMap<SectionLink, usize>,
}

/// The most subjects counted at once, so one question stays small however many subjects there are.
pub const COUNTED_AT_ONCE: usize = 100;

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

    /// The feedback given on a subject that the agent may see, oldest first:
    /// what everyone submitted, and their own drafts.
    pub async fn feedback_on(
        &self,
        subject: &Subject,
        agent: &Agent,
    ) -> Result<Vec<Feedback>, FeedbackError> {
        // TODO: only for whoever may read the subject's project (step 5)
        Ok(self
            .kept()
            .iter()
            .filter(|feedback| {
                &feedback.about.subject() == subject
                    && (!feedback.is_draft() || &feedback.written_by == agent)
            })
            .cloned()
            .collect())
    }

    /// How much feedback each subject holds, in the order asked, at most `COUNTED_AT_ONCE` of them.
    pub async fn counted_on(
        &self,
        subjects: Vec<Subject>,
        agent: &Agent,
    ) -> Result<Vec<Counted>, FeedbackError> {
        // TODO: only on subjects whose project the agent may read (step 5)
        if subjects.len() > COUNTED_AT_ONCE {
            return Err(FeedbackError::TooManyCounted);
        }
        let kept = self.kept();

        Ok(subjects
            .into_iter()
            .map(|subject| {
                let on: Vec<&Feedback> = kept
                    .iter()
                    .filter(|feedback| feedback.about.subject() == subject)
                    .collect();

                Counted {
                    not_done: on
                        .iter()
                        .filter(|feedback| !feedback.is_draft() && !feedback.done)
                        .count(),
                    your_drafts: on
                        .iter()
                        .filter(|feedback| feedback.is_draft() && &feedback.written_by == agent)
                        .count(),
                    subject,
                }
            })
            .collect())
    }

    /// The feedback given on a scene, in every copy that holds it, oldest first:
    /// on the scene itself, or on a passage in it. Drafts are left out, as only their writers see them.
    pub async fn given_on_scene(
        &self,
        scene: &SceneLink,
        _agent: &Agent,
    ) -> Result<Vec<Feedback>, FeedbackError> {
        // TODO: only in copies of a project the agent may read (step 5)
        Ok(self
            .kept()
            .iter()
            .filter(|feedback| !feedback.is_draft() && feedback.about.in_scene() == Some(scene))
            .cloned()
            .collect())
    }

    /// The feedback given on a section itself, in every copy that holds it, oldest first; drafts are left out.
    pub async fn given_on_section(
        &self,
        section: &SectionLink,
        _agent: &Agent,
    ) -> Result<Vec<Feedback>, FeedbackError> {
        // TODO: only in copies of a project the agent may read (step 5)
        Ok(self
            .kept()
            .iter()
            .filter(|feedback| !feedback.is_draft() && feedback.about.on_section() == Some(section))
            .cloned()
            .collect())
    }

    /// How much feedback given is pending, not done yet, on each scene and section of a project, in every copy of it.
    pub async fn pending_in(
        &self,
        _project: &str,
        _agent: &Agent,
    ) -> Result<Pending, FeedbackError> {
        // TODO: only in the project's copies, for whoever may read it (step 5); its scenes' and sections' ids are its own meanwhile
        let mut pending = Pending::default();
        for feedback in self
            .kept()
            .iter()
            .filter(|feedback| !feedback.is_draft() && !feedback.done)
        {
            if let Some(scene) = feedback.about.in_scene() {
                *pending.scenes.entry(scene.clone()).or_insert(0) += 1;
            }
            if let Some(section) = feedback.about.on_section() {
                *pending.sections.entry(section.clone()).or_insert(0) += 1;
            }
        }

        Ok(pending)
    }

    /// Submits the agent's drafts on a subject together, so everyone who may see the subject's feedback sees them.
    /// It answers with what was submitted, oldest first; with no drafts, nothing.
    pub async fn submit_on(
        &self,
        subject: &Subject,
        agent: &Agent,
    ) -> Result<Vec<Feedback>, FeedbackError> {
        // TODO: only while the subject is open, for whoever may read its project (step 5)
        let now = self.clock.now();
        let mut kept = self.kept();
        let mut submitted = Vec::new();
        for feedback in kept.iter_mut().filter(|feedback| {
            feedback.is_draft()
                && &feedback.about.subject() == subject
                && &feedback.written_by == agent
        }) {
            feedback.submitted_at = Some(now);
            submitted.push(feedback.clone());
        }

        Ok(submitted)
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
            submitted_at: None,
            edited: false,
            done: false,
            written_by: agent.clone(),
            written_at: now,
        };
        self.kept().push(drafted.clone());

        Ok(drafted)
    }

    /// Rewrites what a piece of feedback says, for whoever wrote it; once submitted, it is marked edited.
    pub async fn rewrite(
        &self,
        feedback: &str,
        comment: Vec<CommentBlock>,
        agent: &Agent,
    ) -> Result<Feedback, FeedbackError> {
        let comment = Comment::new(comment)?;
        let mut kept = self.kept();
        let rewritten = written_by(&mut kept, feedback, agent)?;
        rewritten.edited |= !rewritten.is_draft();
        rewritten.comment = comment;

        Ok(rewritten.clone())
    }

    /// Marks a piece of feedback submitted as done, or not done again.
    /// A draft is not there for this: only feedback given is worked through.
    pub async fn mark_done(
        &self,
        feedback: &str,
        done: bool,
        _agent: &Agent,
    ) -> Result<Feedback, FeedbackError> {
        // TODO: only for whoever may write the subject's project, its authors and owners (step 5)
        let mut kept = self.kept();
        let found = kept
            .iter_mut()
            .find(|kept| kept.id.to_string() == feedback)
            .filter(|kept| !kept.is_draft())
            .ok_or(FeedbackError::FeedbackNotThere)?;
        found.done = done;

        Ok(found.clone())
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
        return Err(if found.is_draft() {
            FeedbackError::FeedbackNotThere
        } else {
            FeedbackError::NotYours
        });
    }

    Ok(found)
}
