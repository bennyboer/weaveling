use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};

use clock::Clock;
use eventsourcing::Agent;
use time::OffsetDateTime;

use crate::about::{About, Subject};
use crate::comment::{Comment, CommentBlock};
use crate::error::FeedbackError;
use crate::id::FeedbackId;
use crate::link::{ReadingCopyLink, SceneLink, SectionLink};

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
    /// All of it, on a whole copy too, which lies on no scene and no section.
    pub total: usize,
}

/// In which order feedback submitted is listed, by when it was submitted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Order {
    #[default]
    Oldest,
    Newest,
}

/// One page of a listing: in which order, after which piece it goes on, and how many at most.
/// By default all of it, oldest first; never more than `COUNTED_AT_ONCE` at once when a page is asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Page {
    pub order: Order,
    pub after: Option<String>,
    pub at_most: Option<usize>,
}

/// Which feedback is asked for: in a project, on some subjects, at some places in them.
/// Each narrows what the others leave; what is not named does not narrow, so nothing named would be everything.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Asked {
    pub project: Option<String>,
    pub subjects: Option<Vec<Subject>>,
    pub places: Option<Places>,
}

/// Places inside subjects, as each kind of subject has them: in a reading copy's text,
/// some scenes, on them or passages in them, and some sections themselves, feedback on any of them found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Places {
    Text {
        scenes: Vec<SceneLink>,
        sections: Vec<SectionLink>,
    },
}

impl Places {
    fn len(&self) -> usize {
        match self {
            Self::Text { scenes, sections } => scenes.len() + sections.len(),
        }
    }

    fn hold(&self, about: &About) -> bool {
        match self {
            Self::Text { scenes, sections } => {
                about.in_scene().is_some_and(|scene| scenes.contains(scene))
                    || about
                        .on_section()
                        .is_some_and(|section| sections.contains(section))
            }
        }
    }
}

impl Asked {
    /// How many subjects and places are looked on at once; a project counts as none.
    fn looked_on(&self) -> usize {
        self.subjects.as_ref().map_or(0, Vec::len) + self.places.as_ref().map_or(0, Places::len)
    }

    fn holds(&self, about: &About, copies: &HashMap<ReadingCopyLink, String>) -> bool {
        self.project
            .as_ref()
            .is_none_or(|project| in_project(about, project, copies))
            && self
                .subjects
                .as_ref()
                .is_none_or(|subjects| subjects.contains(&about.subject()))
            && self.places.as_ref().is_none_or(|places| places.hold(about))
    }
}

fn in_project(about: &About, project: &str, copies: &HashMap<ReadingCopyLink, String>) -> bool {
    match about.subject() {
        Subject::ReadingCopy(copy) => copies.get(&copy).is_some_and(|made_of| made_of == project),
    }
}

/// Whether feedback given is still to be worked through, or done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pending,
    Done,
}

impl Status {
    fn holds(self, feedback: &Feedback) -> bool {
        feedback.done == (self == Self::Done)
    }
}

/// The most subjects counted at once, so one question stays small however many subjects there are.
pub const COUNTED_AT_ONCE: usize = 100;

/// The feedback feature's only way in.
#[derive(Clone)]
pub struct FeedbackCore {
    // TODO: the `Feedback` aggregate in an event store and its catalogs, rather than what this process holds (step 5)
    kept: Arc<Mutex<Vec<Feedback>>>,
    copies: Arc<Mutex<HashMap<ReadingCopyLink, String>>>,
    clock: Arc<dyn Clock>,
}

impl FeedbackCore {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            kept: Arc::new(Mutex::new(Vec::new())),
            copies: Arc::new(Mutex::new(HashMap::new())),
            clock,
        }
    }

    fn kept(&self) -> MutexGuard<'_, Vec<Feedback>> {
        self.kept.lock().expect("feedback lock poisoned")
    }

    fn copies(&self) -> HashMap<ReadingCopyLink, String> {
        self.copies.lock().expect("copies lock poisoned").clone()
    }

    /// Remembers which project a reading copy was made of, so feedback on it is found in that project.
    pub fn remember_copy(&self, copy: ReadingCopyLink, project: &str) {
        self.copies
            .lock()
            .expect("copies lock poisoned")
            .insert(copy, project.to_owned());
    }

    /// The agent's own drafts where asked, oldest first, which only they see until they submit them.
    /// At most `COUNTED_AT_ONCE` subjects and places are looked on at once.
    pub async fn drafts(
        &self,
        asked: &Asked,
        agent: &Agent,
    ) -> Result<Vec<Feedback>, FeedbackError> {
        if asked.looked_on() > COUNTED_AT_ONCE {
            return Err(FeedbackError::TooManyCounted);
        }
        let copies = self.copies();

        Ok(self
            .kept()
            .iter()
            .filter(|feedback| {
                feedback.is_draft()
                    && &feedback.written_by == agent
                    && asked.holds(&feedback.about, &copies)
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

    /// The feedback submitted where asked, in every copy, oldest first, and if asked only what is pending or done.
    /// Drafts are left out, as only their writers see them. At most `COUNTED_AT_ONCE` subjects and places are looked on at once,
    /// so the writing view asks only for those in sight.
    pub async fn submitted(
        &self,
        asked: &Asked,
        status: Option<Status>,
        page: &Page,
        _agent: &Agent,
    ) -> Result<Vec<Feedback>, FeedbackError> {
        // TODO: only in copies of a project the agent may read (step 5)
        if asked.looked_on() > COUNTED_AT_ONCE
            || page.at_most.is_some_and(|most| most > COUNTED_AT_ONCE)
        {
            return Err(FeedbackError::TooManyCounted);
        }
        let copies = self.copies();
        let mut found: Vec<Feedback> = self
            .kept()
            .iter()
            .filter(|feedback| {
                !feedback.is_draft()
                    && asked.holds(&feedback.about, &copies)
                    && status.is_none_or(|status| status.holds(feedback))
            })
            .cloned()
            .collect();
        found.sort_by_key(|piece| piece.submitted_at);
        if page.order == Order::Newest {
            found.reverse();
        }
        let from = page
            .after
            .as_ref()
            .and_then(|after| {
                found
                    .iter()
                    .position(|piece| &piece.id.to_string() == after)
            })
            .map_or(0, |at| at + 1);

        Ok(found
            .into_iter()
            .skip(from)
            .take(page.at_most.unwrap_or(usize::MAX))
            .collect())
    }

    /// How much feedback given is pending, not done yet, on each scene and section of a project, in every copy of it.
    pub async fn pending_in(
        &self,
        project: &str,
        _agent: &Agent,
    ) -> Result<Pending, FeedbackError> {
        // TODO: for whoever may read the project (step 5)
        let copies = self.copies();
        let mut pending = Pending::default();
        for feedback in self.kept().iter().filter(|feedback| {
            !feedback.is_draft() && !feedback.done && in_project(&feedback.about, project, &copies)
        }) {
            if let Some(scene) = feedback.about.in_scene() {
                *pending.scenes.entry(scene.clone()).or_insert(0) += 1;
            }
            if let Some(section) = feedback.about.on_section() {
                *pending.sections.entry(section.clone()).or_insert(0) += 1;
            }
            pending.total += 1;
        }

        Ok(pending)
    }

    /// Submits the agent's drafts where asked together, so everyone who may see the subject's feedback sees them.
    /// It answers with what was submitted, oldest first; with no drafts, nothing.
    pub async fn submit(
        &self,
        asked: &Asked,
        agent: &Agent,
    ) -> Result<Vec<Feedback>, FeedbackError> {
        // TODO: only while the subject is open, for whoever may read its project (step 5)
        if asked.looked_on() > COUNTED_AT_ONCE {
            return Err(FeedbackError::TooManyCounted);
        }
        let now = self.clock.now();
        let copies = self.copies();
        let mut kept = self.kept();
        let mut submitted = Vec::new();
        for feedback in kept.iter_mut().filter(|feedback| {
            feedback.is_draft()
                && &feedback.written_by == agent
                && asked.holds(&feedback.about, &copies)
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
