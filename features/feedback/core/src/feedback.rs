use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};

use clock::Clock;
use eventsourcing::{AccountId, Agent};
use time::OffsetDateTime;
use transacting::{Transaction, Transactions};

use crate::about::{About, Subject};
use crate::catalog::{
    AccountNameCatalog, CataloguedFeedback, KnownReadingCopy, ReadingCopyCatalog,
};
use crate::comment::{Comment, CommentBlock};
use crate::error::FeedbackError;
use crate::id::FeedbackId;
use crate::link::{ReadingCopyLink, SceneLink, SectionLink};

/// A piece of feedback, as whoever may see it finds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedFeedback {
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
    /// The name its writer signed up with, where it is known; none once their account is deleted.
    pub written_by_name: Option<String>,
    pub written_at: OffsetDateTime,
}

/// Whoever gave feedback, and their name where it is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Giver {
    pub agent: Agent,
    pub name: Option<String>,
}

impl ListedFeedback {
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

/// Which feedback is asked for: in a project, on some subjects, at some places in them, given by some.
/// Each narrows what the others leave; what is not named does not narrow, so nothing named would be everything.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Asked {
    pub project: Option<String>,
    pub subjects: Option<Vec<Subject>>,
    pub places: Option<Places>,
    pub given_by: Option<Vec<Agent>>,
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

    fn holds(
        &self,
        feedback: &ListedFeedback,
        copies: &HashMap<ReadingCopyLink, KnownReadingCopy>,
    ) -> bool {
        let about = &feedback.about;

        self.project
            .as_ref()
            .is_none_or(|project| in_project(about, project, copies))
            && self
                .subjects
                .as_ref()
                .is_none_or(|subjects| subjects.contains(&about.subject()))
            && self.places.as_ref().is_none_or(|places| places.hold(about))
            && self
                .given_by
                .as_ref()
                .is_none_or(|givers| givers.contains(&feedback.written_by))
    }
}

impl Asked {
    /// Whether a piece kept in the catalog is among those asked for: in the project, on a subject, at a place and by a giver named.
    pub fn holds_catalogued(&self, piece: &CataloguedFeedback) -> bool {
        self.project
            .as_ref()
            .is_none_or(|project| piece.project.as_str() == project)
            && self
                .subjects
                .as_ref()
                .is_none_or(|subjects| subjects.contains(&piece.about.subject()))
            && self
                .places
                .as_ref()
                .is_none_or(|places| places.hold(&piece.about))
            && self
                .given_by
                .as_ref()
                .is_none_or(|givers| givers.contains(&piece.written_by))
    }
}

fn in_project(
    about: &About,
    project: &str,
    copies: &HashMap<ReadingCopyLink, KnownReadingCopy>,
) -> bool {
    match about.subject() {
        Subject::ReadingCopy(copy) => copies
            .get(&copy)
            .is_some_and(|known| known.project.as_str() == project),
    }
}

fn name_in(names: &HashMap<AccountId, String>, agent: &Agent) -> Option<String> {
    match agent {
        Agent::Account(account) => names.get(account).cloned(),
        Agent::Local | Agent::System => None,
    }
}

fn reading_copy_of(about: &About) -> ReadingCopyLink {
    match about.subject() {
        Subject::ReadingCopy(copy) => copy,
    }
}

/// Whether feedback given is still to be worked through, or done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pending,
    Done,
}

impl Status {
    fn holds(self, feedback: &ListedFeedback) -> bool {
        feedback.done == (self == Self::Done)
    }

    /// Whether a piece kept in the catalog is pending or done as asked.
    pub fn holds_catalogued(self, piece: &CataloguedFeedback) -> bool {
        piece.done == (self == Self::Done)
    }
}

/// The most subjects counted at once, so one question stays small however many subjects there are.
pub const COUNTED_AT_ONCE: usize = 100;

/// The feedback feature's only way in.
#[derive(Clone)]
pub struct FeedbackCore {
    // TODO: the `Feedback` aggregate in an event store and its catalogs, rather than what this process holds (step 5)
    kept: Arc<Mutex<Vec<ListedFeedback>>>,
    copies: Arc<dyn ReadingCopyCatalog>,
    names: Arc<dyn AccountNameCatalog>,
    transactions: Arc<dyn Transactions>,
    clock: Arc<dyn Clock>,
}

impl FeedbackCore {
    pub fn new(
        copies: Arc<dyn ReadingCopyCatalog>,
        names: Arc<dyn AccountNameCatalog>,
        transactions: Arc<dyn Transactions>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            kept: Arc::new(Mutex::new(Vec::new())),
            copies,
            names,
            transactions,
            clock,
        }
    }

    /// The names of the accounts among these agents, where they are known.
    async fn names_of<'a>(
        &self,
        agents: impl Iterator<Item = &'a Agent>,
    ) -> Result<HashMap<AccountId, String>, FeedbackError> {
        let mut accounts: Vec<AccountId> = agents
            .filter_map(|agent| match agent {
                Agent::Account(account) => Some(account.clone()),
                Agent::Local | Agent::System => None,
            })
            .collect();
        accounts.sort_by_key(ToString::to_string);
        accounts.dedup();

        Ok(self.names.names_of(&accounts).await?)
    }

    /// The pieces, each with its writer's name where it is known.
    async fn named(
        &self,
        mut pieces: Vec<ListedFeedback>,
    ) -> Result<Vec<ListedFeedback>, FeedbackError> {
        let names = self
            .names_of(pieces.iter().map(|piece| &piece.written_by))
            .await?;
        for piece in &mut pieces {
            piece.written_by_name = name_in(&names, &piece.written_by);
        }

        Ok(pieces)
    }

    async fn named_one(&self, piece: ListedFeedback) -> Result<ListedFeedback, FeedbackError> {
        let mut named = self.named(vec![piece]).await?;

        Ok(named.remove(0))
    }

    /// Keeps the name an account signed up with, so its feedback names it.
    pub async fn name_heard(
        &self,
        within: Option<&mut Transaction>,
        account: &AccountId,
        name: &str,
    ) -> Result<(), FeedbackError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.names.remember_name(transaction, account, name).await?;

            Ok(())
        })
    }

    /// Forgets a deleted account's name, so its feedback names nobody from now on.
    pub async fn account_gone(
        &self,
        within: Option<&mut Transaction>,
        account: &AccountId,
        at: OffsetDateTime,
    ) -> Result<(), FeedbackError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.names.forget_name(transaction, account, at).await?;

            Ok(())
        })
    }

    fn kept(&self) -> MutexGuard<'_, Vec<ListedFeedback>> {
        self.kept.lock().expect("feedback lock poisoned")
    }

    /// The copies the feedback kept is given on, as the readings told of them.
    async fn reading_copies_heard(
        &self,
    ) -> Result<HashMap<ReadingCopyLink, KnownReadingCopy>, FeedbackError> {
        let mut given_on: Vec<ReadingCopyLink> = self
            .kept()
            .iter()
            .map(|feedback| reading_copy_of(&feedback.about))
            .collect();
        given_on.sort();
        given_on.dedup();

        Ok(self.copies.known(&given_on).await?)
    }

    /// Refuses feedback on a copy the readings told is closed by now.
    /// A copy not heard of yet is let through, as feedback is drafted right after a copy is made for it.
    async fn open_for_feedback(&self, copies: &[ReadingCopyLink]) -> Result<(), FeedbackError> {
        let now = self.clock.now();
        let known = self.copies.known(copies).await?;
        if known.values().any(|copy| !copy.is_open_at(now)) {
            return Err(FeedbackError::ReadingCopyClosed);
        }

        Ok(())
    }

    /// Keeps a reading copy as the readings told it at a version of it: its project, and until when it is open.
    pub async fn reading_copy_heard(
        &self,
        within: Option<&mut Transaction>,
        copy: &ReadingCopyLink,
        known: &KnownReadingCopy,
        version: u64,
    ) -> Result<(), FeedbackError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.copies.keep(transaction, copy, known, version).await?;

            Ok(())
        })
    }

    /// Forgets a deleted reading copy, so a late message about it does not bring it back.
    pub async fn reading_copy_deleted(
        &self,
        within: Option<&mut Transaction>,
        copy: &ReadingCopyLink,
        at: OffsetDateTime,
    ) -> Result<(), FeedbackError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.copies.forget(transaction, copy, at).await?;

            Ok(())
        })
    }

    /// Forgets, in batches, the tombstones laid long enough ago that no message about them can still arrive:
    /// those of deleted reading copies, and of deleted accounts' names.
    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, FeedbackError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            let reading_copies = self
                .copies
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;
            let names = self
                .names
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;

            Ok(reading_copies + names)
        })
    }

    /// The agent's own drafts where asked, oldest first, which only they see until they submit them.
    /// At most `COUNTED_AT_ONCE` subjects and places are looked on at once.
    pub async fn drafts(
        &self,
        asked: &Asked,
        agent: &Agent,
    ) -> Result<Vec<ListedFeedback>, FeedbackError> {
        if asked.looked_on() > COUNTED_AT_ONCE {
            return Err(FeedbackError::TooManyCounted);
        }
        let copies = self.reading_copies_heard().await?;
        let found: Vec<ListedFeedback> = self
            .kept()
            .iter()
            .filter(|feedback| {
                feedback.is_draft()
                    && &feedback.written_by == agent
                    && asked.holds(feedback, &copies)
            })
            .cloned()
            .collect();

        self.named(found).await
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
                let on: Vec<&ListedFeedback> = kept
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
    ) -> Result<Vec<ListedFeedback>, FeedbackError> {
        // TODO: only in copies of a project the agent may read (step 5)
        if asked.looked_on() > COUNTED_AT_ONCE
            || page.at_most.is_some_and(|most| most > COUNTED_AT_ONCE)
        {
            return Err(FeedbackError::TooManyCounted);
        }
        let copies = self.reading_copies_heard().await?;
        let mut found: Vec<ListedFeedback> = self
            .kept()
            .iter()
            .filter(|feedback| {
                !feedback.is_draft()
                    && asked.holds(feedback, &copies)
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

        let paged = found
            .into_iter()
            .skip(from)
            .take(page.at_most.unwrap_or(usize::MAX))
            .collect();

        self.named(paged).await
    }

    /// Whoever gave feedback in a project, in every copy of it, in the order they first submitted any, each with their name where it is known.
    pub async fn givers_in(
        &self,
        project: &str,
        _agent: &Agent,
    ) -> Result<Vec<Giver>, FeedbackError> {
        // TODO: for whoever may read the project (step 5)
        let copies = self.reading_copies_heard().await?;
        let givers: Vec<Agent> = {
            let kept = self.kept();
            let mut given: Vec<&ListedFeedback> = kept
                .iter()
                .filter(|feedback| {
                    !feedback.is_draft() && in_project(&feedback.about, project, &copies)
                })
                .collect();
            given.sort_by_key(|feedback| feedback.submitted_at);
            let mut givers: Vec<Agent> = Vec::new();
            for feedback in given {
                if !givers.contains(&feedback.written_by) {
                    givers.push(feedback.written_by.clone());
                }
            }

            givers
        };
        let names = self.names_of(givers.iter()).await?;

        Ok(givers
            .into_iter()
            .map(|agent| Giver {
                name: name_in(&names, &agent),
                agent,
            })
            .collect())
    }

    /// How much feedback given where asked is pending, not done yet: on each scene and section, and in all.
    pub async fn pending(&self, asked: &Asked, _agent: &Agent) -> Result<Pending, FeedbackError> {
        // TODO: only in copies of a project the agent may read (step 5)
        if asked.looked_on() > COUNTED_AT_ONCE {
            return Err(FeedbackError::TooManyCounted);
        }
        let copies = self.reading_copies_heard().await?;
        let mut pending = Pending::default();
        for feedback in self.kept().iter().filter(|feedback| {
            !feedback.is_draft() && !feedback.done && asked.holds(feedback, &copies)
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
    ) -> Result<Vec<ListedFeedback>, FeedbackError> {
        // TODO: for whoever may read its project (step 5)
        if asked.looked_on() > COUNTED_AT_ONCE {
            return Err(FeedbackError::TooManyCounted);
        }
        let copies = self.reading_copies_heard().await?;
        let submitting: Vec<ReadingCopyLink> = self
            .kept()
            .iter()
            .filter(|feedback| {
                feedback.is_draft()
                    && &feedback.written_by == agent
                    && asked.holds(feedback, &copies)
            })
            .map(|feedback| reading_copy_of(&feedback.about))
            .collect();
        self.open_for_feedback(&submitting).await?;
        let now = self.clock.now();
        let submitted = {
            let mut kept = self.kept();
            let mut submitted = Vec::new();
            for feedback in kept.iter_mut().filter(|feedback| {
                feedback.is_draft()
                    && &feedback.written_by == agent
                    && asked.holds(feedback, &copies)
            }) {
                feedback.submitted_at = Some(now);
                submitted.push(feedback.clone());
            }

            submitted
        };

        self.named(submitted).await
    }

    /// Starts a draft about something, which only whoever writes it sees until they submit it.
    pub async fn draft(
        &self,
        about: About,
        comment: Vec<CommentBlock>,
        agent: &Agent,
    ) -> Result<ListedFeedback, FeedbackError> {
        // TODO: for whoever may read its project (step 5)
        let comment = Comment::new(comment)?;
        if !about.holds_something() {
            return Err(FeedbackError::PassageEmpty);
        }
        self.open_for_feedback(&[reading_copy_of(&about)]).await?;
        let now = self.clock.now();
        let drafted = ListedFeedback {
            id: FeedbackId::generate(now),
            about,
            comment,
            submitted_at: None,
            edited: false,
            done: false,
            written_by: agent.clone(),
            written_by_name: None,
            written_at: now,
        };
        self.kept().push(drafted.clone());

        self.named_one(drafted).await
    }

    /// Rewrites what a piece of feedback says, for whoever wrote it; once submitted, it is marked edited.
    pub async fn rewrite(
        &self,
        feedback: &str,
        comment: Vec<CommentBlock>,
        agent: &Agent,
    ) -> Result<ListedFeedback, FeedbackError> {
        let comment = Comment::new(comment)?;
        let rewritten = {
            let mut kept = self.kept();
            let rewritten = written_by(&mut kept, feedback, agent)?;
            rewritten.edited |= !rewritten.is_draft();
            rewritten.comment = comment;

            rewritten.clone()
        };

        self.named_one(rewritten).await
    }

    /// Marks a piece of feedback submitted as done, or not done again.
    /// A draft is not there for this: only feedback given is worked through.
    pub async fn mark_done(
        &self,
        feedback: &str,
        done: bool,
        _agent: &Agent,
    ) -> Result<ListedFeedback, FeedbackError> {
        // TODO: only for whoever may write the subject's project, its authors and owners (step 5)
        let marked = {
            let mut kept = self.kept();
            let found = kept
                .iter_mut()
                .find(|kept| kept.id.to_string() == feedback)
                .filter(|kept| !kept.is_draft())
                .ok_or(FeedbackError::FeedbackNotThere)?;
            found.done = done;

            found.clone()
        };

        self.named_one(marked).await
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
    kept: &'a mut [ListedFeedback],
    feedback: &str,
    agent: &Agent,
) -> Result<&'a mut ListedFeedback, FeedbackError> {
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
