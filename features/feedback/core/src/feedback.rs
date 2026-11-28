use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;
use std::sync::Arc;

use ::access::KeptAccess;
use authorizing::{AccessWatch, Act, Authorizer};
use clock::Clock;
use eventsourcing::{AccountId, Agent, AggregateId, EventSourcingService, EventStore};
use time::OffsetDateTime;
use transacting::{Transaction, Transactions};

use crate::about::{About, Subject};
use crate::catalog::{
    AccountNameCatalog, CataloguedFeedback, FeedbackCatalog, KnownReadingCopy, ReadingCopyCatalog,
};
use crate::comment::{Comment, CommentBlock};
use crate::error::FeedbackError;
use crate::following::FeedbackFollowers;
use crate::id::FeedbackId;
use crate::link::{ProjectLink, ReadingCopyLink, SceneLink, SectionLink};
use crate::piece::{Deleter, Feedback, FeedbackCommand, FeedbackEvent};

/// A piece of feedback, as whoever may see it finds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedFeedback {
    pub id: FeedbackId,
    pub project: ProjectLink,
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
    /// How many replies are given on it.
    pub replies: usize,
}

impl ListedFeedback {
    pub fn is_draft(&self) -> bool {
        self.submitted_at.is_none()
    }

    fn of(
        piece: CataloguedFeedback,
        names: &HashMap<AccountId, String>,
        replies: &HashMap<FeedbackId, usize>,
    ) -> Self {
        Self {
            written_by_name: name_in(names, &piece.written_by),
            replies: replies.get(&piece.id).copied().unwrap_or(0),
            id: piece.id,
            project: piece.project,
            about: piece.about,
            comment: piece.comment,
            submitted_at: piece.submitted_at,
            edited: piece.edited,
            done: piece.done,
            written_by: piece.written_by,
            written_at: piece.written_at,
        }
    }
}

impl CataloguedFeedback {
    /// The piece as the catalog keeps it, while it still holds its words; a deleted one is kept no more.
    fn of(id: FeedbackId, piece: &Feedback) -> Option<Self> {
        Some(Self {
            id,
            project: piece.project().clone(),
            about: piece.about()?.clone(),
            comment: piece.comment()?.clone(),
            written_by: piece.written_by().clone(),
            written_at: piece.written_at(),
            submitted_at: piece.submitted_at(),
            edited: piece.is_edited(),
            done: piece.is_done(),
        })
    }
}

/// Whoever gave feedback, and their name where it is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Giver {
    pub agent: Agent,
    pub name: Option<String>,
}

/// How much feedback a subject holds, as one who may see it counts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counted {
    pub subject: Subject,
    /// The feedback submitted, done or not; on a piece of feedback, its replies.
    pub submitted: usize,
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

    /// Whether a piece is among those asked for: in the project, on a subject, at a place and by a giver named.
    /// With no subjects named it is feedback on the work alone, as replies are asked for by the piece they are given on.
    pub fn holds(&self, piece: &CataloguedFeedback) -> bool {
        let subject = piece.about.subject();

        self.project
            .as_ref()
            .is_none_or(|project| piece.project.as_str() == project)
            && self
                .subjects
                .as_ref()
                .map_or(!subject.is_feedback(), |subjects| {
                    subjects.contains(&subject)
                })
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

/// Whether feedback given is still to be worked through, or done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pending,
    Done,
}

impl Status {
    /// Whether a piece is pending or done as asked.
    pub fn holds(self, piece: &CataloguedFeedback) -> bool {
        piece.done == (self == Self::Done)
    }
}

fn name_in(names: &HashMap<AccountId, String>, agent: &Agent) -> Option<String> {
    match agent {
        Agent::Account(account) => names.get(account).cloned(),
        Agent::Local | Agent::System => None,
    }
}

/// The reading copy a piece is given in; none for a reply, given on another piece.
fn reading_copy_of(about: &About) -> Option<ReadingCopyLink> {
    match about.subject() {
        Subject::ReadingCopy(copy) => Some(copy),
        Subject::Feedback(_) => None,
    }
}

/// The most subjects counted at once, so one question stays small however many subjects there are.
pub const COUNTED_AT_ONCE: usize = 100;

/// How many pieces of a deleted reading copy are found at once to be swept.
const SWEPT_AT_ONCE: usize = 100;

/// What of a reading copy's feedback is swept: all of it as the copy is deleted, or the drafts left as it closes.
#[derive(Clone, Copy)]
enum Swept {
    Everything,
    Drafts,
}

/// The feedback feature's only way in.
#[derive(Clone)]
pub struct FeedbackCore {
    events: Arc<EventSourcingService<Feedback>>,
    feedback: Arc<dyn FeedbackCatalog>,
    copies: Arc<dyn ReadingCopyCatalog>,
    names: Arc<dyn AccountNameCatalog>,
    followers: Arc<dyn FeedbackFollowers>,
    pub(crate) transactions: Arc<dyn Transactions>,
    pub(crate) access: KeptAccess,
    authorizer: Arc<dyn Authorizer>,
    pub(crate) clock: Arc<dyn Clock>,
}

impl FeedbackCore {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        events: Arc<dyn EventStore<FeedbackEvent>>,
        feedback: Arc<dyn FeedbackCatalog>,
        copies: Arc<dyn ReadingCopyCatalog>,
        names: Arc<dyn AccountNameCatalog>,
        followers: Arc<dyn FeedbackFollowers>,
        transactions: Arc<dyn Transactions>,
        access: KeptAccess,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(events, clock.clone())),
            feedback,
            copies,
            names,
            followers,
            transactions,
            access,
            authorizer,
            clock,
        }
    }

    /// Whether the agent may follow a project's feedback live: whoever may read the project may.
    pub async fn may_follow(&self, project: &str, agent: &Agent) -> Result<bool, FeedbackError> {
        Ok(self.authorizer.may(agent, Act::Read, project).await?)
    }

    pub fn watch_access(&self, project: &str) -> AccessWatch {
        self.authorizer.watch_access(project)
    }

    /// Refuses unless the agent may act so in the project; a project they may not even read is not there for them.
    async fn allowed(
        &self,
        agent: &Agent,
        act: Act,
        project: &ProjectLink,
        refused: FeedbackError,
    ) -> Result<(), FeedbackError> {
        if !self
            .authorizer
            .may(agent, Act::Read, project.as_str())
            .await?
        {
            return Err(FeedbackError::ProjectNotThere);
        }
        if act != Act::Read && !self.authorizer.may(agent, act, project.as_str()).await? {
            return Err(refused);
        }

        Ok(())
    }

    /// Refuses unless the agent may read every project the feedback asked for lies in:
    /// the project named, or those the named reading copies were made of. A copy not heard of yet holds no feedback.
    /// Feedback asked for in neither could lie in any project, and is refused.
    async fn readable(&self, asked: &Asked, agent: &Agent) -> Result<(), FeedbackError> {
        let projects: Vec<ProjectLink> = match (&asked.project, &asked.subjects) {
            (Some(project), _) => vec![ProjectLink::from(project.as_str())],
            (None, Some(subjects)) => self.projects_of(subjects).await?,
            (None, None) => return Err(FeedbackError::ProjectUnnamed),
        };
        for project in &projects {
            self.allowed(agent, Act::Read, project, FeedbackError::ProjectNotThere)
                .await?;
        }

        Ok(())
    }

    /// The projects the subjects lie in, as far as the feedback heard of them.
    async fn projects_of(&self, subjects: &[Subject]) -> Result<Vec<ProjectLink>, FeedbackError> {
        let copies: Vec<ReadingCopyLink> = subjects
            .iter()
            .filter_map(|subject| match subject {
                Subject::ReadingCopy(copy) => Some(copy.clone()),
                Subject::Feedback(_) => None,
            })
            .collect();
        let mut projects: Vec<ProjectLink> = self
            .copies
            .known(&copies)
            .await?
            .into_values()
            .map(|copy| copy.project)
            .collect();
        for subject in subjects {
            if let Subject::Feedback(piece) = subject
                && let Some(piece) = self.feedback.piece(piece).await?
            {
                projects.push(piece.project);
            }
        }
        projects.sort();
        projects.dedup();

        Ok(projects)
    }

    /// The name an agent signed up with, where it is known.
    pub async fn name_of(&self, agent: &Agent) -> Result<Option<String>, FeedbackError> {
        let names = self.names_of(std::iter::once(agent)).await?;

        Ok(name_in(&names, agent))
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

    /// The pieces as listed, each with its writer's name where it is known, and the comments given with how many replies they have.
    async fn listed(
        &self,
        pieces: Vec<CataloguedFeedback>,
    ) -> Result<Vec<ListedFeedback>, FeedbackError> {
        let names = self
            .names_of(pieces.iter().map(|piece| &piece.written_by))
            .await?;
        let given_on: Vec<Subject> = pieces
            .iter()
            .filter(|piece| !piece.is_draft() && piece.about.replying_to().is_none())
            .map(|piece| Subject::Feedback(piece.id))
            .collect();
        let replies: HashMap<FeedbackId, usize> = match given_on.is_empty() {
            true => HashMap::new(),
            false => self
                .feedback
                .counted_on(&given_on, &Agent::System)
                .await?
                .into_iter()
                .filter_map(|counted| match counted.subject {
                    Subject::Feedback(piece) => Some((piece, counted.submitted)),
                    Subject::ReadingCopy(_) => None,
                })
                .collect(),
        };

        Ok(pieces
            .into_iter()
            .map(|piece| ListedFeedback::of(piece, &names, &replies))
            .collect())
    }

    async fn listed_one(&self, piece: CataloguedFeedback) -> Result<ListedFeedback, FeedbackError> {
        let mut listed = self.listed(vec![piece]).await?;

        Ok(listed.remove(0))
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
    /// those of deleted reading copies, of deleted accounts' names, and of accounts that left a project.
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
            let access = self
                .access
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;

            Ok(reading_copies + names + access)
        })
    }

    /// The reading copies feedback is given on, as the readings told of them, while they are open for it.
    /// A copy not heard of yet is not known to be open, nor which project it was made of; it is asked for again shortly.
    async fn open_reading_copies(
        &self,
        copies: &[ReadingCopyLink],
    ) -> Result<HashMap<ReadingCopyLink, KnownReadingCopy>, FeedbackError> {
        let now = self.clock.now();
        let known = self.copies.known(copies).await?;
        if copies.iter().any(|copy| !known.contains_key(copy)) {
            return Err(FeedbackError::ReadingCopyUnknown);
        }
        if known.values().any(|copy| !copy.is_open_at(now)) {
            return Err(FeedbackError::ReadingCopyClosed);
        }

        Ok(known)
    }

    /// Keeps a piece in the catalog as its events now leave it, or forgets it once it is deleted.
    /// Whoever follows its project live is told once it is committed, where it was given and not a draft.
    async fn catalogue(
        &self,
        transaction: &mut Transaction,
        piece: FeedbackId,
    ) -> Result<Option<CataloguedFeedback>, FeedbackError> {
        let standing = self
            .events
            .latest_within(transaction, &AggregateId::from(&piece))
            .await?;
        let Some(catalogued) =
            CataloguedFeedback::of(piece, &standing.state).filter(|_| !standing.state.is_deleted())
        else {
            let forgotten = self.feedback.forget(transaction, &piece).await?;
            if !standing.state.is_draft()
                && let Some(subject) = forgotten
            {
                self.followers
                    .given_gone(transaction, standing.state.project(), &piece, &subject);
            }

            return Ok(None);
        };
        self.feedback.keep(transaction, &catalogued).await?;
        if !catalogued.is_draft() {
            self.followers.given_changed(
                transaction,
                &catalogued.project,
                &catalogued.id,
                &catalogued.about.subject(),
            );
        }

        Ok(Some(catalogued))
    }

    /// A piece as the agent may find it: a draft only by whoever wrote it, feedback submitted by anyone.
    /// Someone else's draft is not there for them at all.
    /// To anyone who may not read its project, it is not there either.
    async fn found(&self, piece: &str, agent: &Agent) -> Result<CataloguedFeedback, FeedbackError> {
        let id = FeedbackId::from_str(piece).map_err(|_| FeedbackError::FeedbackNotThere)?;
        let found = self
            .feedback
            .piece(&id)
            .await?
            .ok_or(FeedbackError::FeedbackNotThere)?;
        if found.is_draft() && &found.written_by != agent {
            return Err(FeedbackError::FeedbackNotThere);
        }
        self.allowed(
            agent,
            Act::Read,
            &found.project,
            FeedbackError::FeedbackNotThere,
        )
        .await
        .map_err(|_| FeedbackError::FeedbackNotThere)?;

        Ok(found)
    }

    /// The piece given a reply goes to, as the replier finds it: the piece named,
    /// or, where that is a reply, the piece its thread began with. A draft is not there, as nobody replies to it.
    async fn thread_root(
        &self,
        to: &str,
        agent: &Agent,
    ) -> Result<CataloguedFeedback, FeedbackError> {
        let named = self.found(to, agent).await?;
        let root = match named.about.replying_to() {
            Some(root) => self.found(&root.to_string(), agent).await?,
            None => named,
        };
        if root.is_draft() {
            return Err(FeedbackError::FeedbackNotThere);
        }

        Ok(root)
    }

    /// Refuses once the reading copy a piece lies in is closed, or not heard of; a piece on anything else is always open.
    async fn still_open_for_feedback(&self, about: &About) -> Result<(), FeedbackError> {
        match reading_copy_of(about) {
            Some(copy) => self
                .open_reading_copies(std::slice::from_ref(&copy))
                .await
                .map(|_| ()),
            None => Ok(()),
        }
    }

    /// Carries out a command on a piece and keeps it in the catalog as it then stands, in one transaction.
    async fn carried_out(
        &self,
        piece: FeedbackId,
        command: FeedbackCommand,
        agent: &Agent,
    ) -> Result<Option<CataloguedFeedback>, FeedbackError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            self.events
                .execute(
                    transaction,
                    &AggregateId::from(&piece),
                    command.clone(),
                    agent,
                )
                .await?;

            self.catalogue(transaction, piece).await
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
        self.readable(asked, agent).await?;

        self.listed(self.feedback.drafts_of(asked, agent).await?)
            .await
    }

    /// How much feedback each subject holds, in the order asked, at most `COUNTED_AT_ONCE` of them.
    pub async fn counted_on(
        &self,
        subjects: Vec<Subject>,
        agent: &Agent,
    ) -> Result<Vec<Counted>, FeedbackError> {
        if subjects.len() > COUNTED_AT_ONCE {
            return Err(FeedbackError::TooManyCounted);
        }
        for project in self.projects_of(&subjects).await? {
            self.allowed(agent, Act::Read, &project, FeedbackError::ProjectNotThere)
                .await?;
        }

        Ok(self.feedback.counted_on(&subjects, agent).await?)
    }

    /// The feedback submitted where asked, in every copy, oldest first, and if asked only what is pending or done.
    /// Drafts are left out, as only their writers see them. At most `COUNTED_AT_ONCE` subjects and places are looked on at once,
    /// so the writing view asks only for those in sight.
    pub async fn submitted(
        &self,
        asked: &Asked,
        status: Option<Status>,
        page: &Page,
        agent: &Agent,
    ) -> Result<Vec<ListedFeedback>, FeedbackError> {
        if asked.looked_on() > COUNTED_AT_ONCE
            || page.at_most.is_some_and(|most| most > COUNTED_AT_ONCE)
        {
            return Err(FeedbackError::TooManyCounted);
        }
        self.readable(asked, agent).await?;

        self.listed(self.feedback.submitted(asked, status, page).await?)
            .await
    }

    /// The pieces asked for by their ids, in the order asked, as the agent finds them:
    /// one not there for them, gone, someone else's draft or in a project they may not read, is left out.
    /// Whoever follows the feedback live asks so for the pieces they heard changed.
    /// At most `COUNTED_AT_ONCE` are asked for at once.
    pub async fn pieces(
        &self,
        ids: &[String],
        agent: &Agent,
    ) -> Result<Vec<ListedFeedback>, FeedbackError> {
        if ids.len() > COUNTED_AT_ONCE {
            return Err(FeedbackError::TooManyCounted);
        }
        let mut found = Vec::new();
        for id in ids {
            match self.found(id, agent).await {
                Ok(piece) => found.push(piece),
                Err(FeedbackError::FeedbackNotThere) => {}
                Err(failure) => return Err(failure),
            }
        }

        self.listed(found).await
    }

    /// Whoever gave feedback in a project, in every copy of it, in the order they first submitted any, each with their name where it is known.
    pub async fn givers_in(
        &self,
        project: &str,
        agent: &Agent,
    ) -> Result<Vec<Giver>, FeedbackError> {
        let project = ProjectLink::from(project);
        self.allowed(agent, Act::Read, &project, FeedbackError::ProjectNotThere)
            .await?;
        let givers = self.feedback.givers_in(&project).await?;
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
    pub async fn pending(&self, asked: &Asked, agent: &Agent) -> Result<Pending, FeedbackError> {
        if asked.looked_on() > COUNTED_AT_ONCE {
            return Err(FeedbackError::TooManyCounted);
        }
        self.readable(asked, agent).await?;

        Ok(self.feedback.pending(asked).await?)
    }

    /// Submits the agent's drafts where asked together, so everyone who may see the subject's feedback sees them.
    /// It answers with what was submitted, oldest first; with no drafts, nothing.
    /// Drafts on a copy closed by now are refused, all of them together.
    pub async fn submit(
        &self,
        asked: &Asked,
        agent: &Agent,
    ) -> Result<Vec<ListedFeedback>, FeedbackError> {
        if asked.looked_on() > COUNTED_AT_ONCE {
            return Err(FeedbackError::TooManyCounted);
        }
        self.readable(asked, agent).await?;
        let drafts = self.feedback.drafts_of(asked, agent).await?;
        let mut copies: Vec<ReadingCopyLink> = drafts
            .iter()
            .filter_map(|draft| reading_copy_of(&draft.about))
            .collect();
        copies.sort();
        copies.dedup();
        self.open_reading_copies(&copies).await?;

        let submitted = transacting::retrying!(&*self.transactions, |transaction| {
            let mut submitted = Vec::new();
            for draft in &drafts {
                self.events
                    .execute(
                        transaction,
                        &AggregateId::from(&draft.id),
                        FeedbackCommand::Submit,
                        agent,
                    )
                    .await?;
                submitted.extend(self.catalogue(transaction, draft.id).await?);
            }

            Ok::<_, FeedbackError>(submitted)
        })?;

        self.listed(submitted).await
    }

    /// Starts a draft about something, which only whoever writes it sees until they submit it,
    /// on a reading copy while it is open.
    pub async fn draft(
        &self,
        about: About,
        comment: Vec<CommentBlock>,
        agent: &Agent,
    ) -> Result<ListedFeedback, FeedbackError> {
        let comment = Comment::new(comment)?;
        if !about.holds_something() {
            return Err(FeedbackError::PassageEmpty);
        }
        let copy = reading_copy_of(&about).ok_or(FeedbackError::RepliesSentAtOnce)?;
        let known = self
            .open_reading_copies(std::slice::from_ref(&copy))
            .await?;
        let project = known
            .get(&copy)
            .map(|copy| copy.project.clone())
            .ok_or(FeedbackError::ReadingCopyUnknown)?;
        self.allowed(agent, Act::Read, &project, FeedbackError::ProjectNotThere)
            .await?;
        let piece = FeedbackId::generate(self.clock.now());

        let drafted = transacting::retrying!(&*self.transactions, |transaction| {
            self.events
                .begin(
                    transaction,
                    &AggregateId::from(&piece),
                    FeedbackCommand::Draft {
                        project: project.clone(),
                        about: about.clone(),
                        comment: comment.clone(),
                    },
                    agent,
                )
                .await?;

            self.catalogue(transaction, piece).await
        })?;

        self.listed_one(drafted.ok_or(FeedbackError::FeedbackNotThere)?)
            .await
    }

    /// Replies on a piece given, sent at once, by whoever may read its project, while the copy it was given in is open.
    /// A reply to a reply is one to the piece that began the thread, as threads on a reading copy are flat.
    pub async fn reply(
        &self,
        to: &str,
        comment: Vec<CommentBlock>,
        agent: &Agent,
    ) -> Result<ListedFeedback, FeedbackError> {
        let comment = Comment::new(comment)?;
        let root = self.thread_root(to, agent).await?;
        self.still_open_for_feedback(&root.about).await?;
        let piece = FeedbackId::generate(self.clock.now());

        let replied = transacting::retrying!(&*self.transactions, |transaction| {
            self.events
                .begin(
                    transaction,
                    &AggregateId::from(&piece),
                    FeedbackCommand::Reply {
                        project: root.project.clone(),
                        to: root.id,
                        comment: comment.clone(),
                    },
                    agent,
                )
                .await?;

            self.catalogue(transaction, piece).await
        })?;

        self.listed_one(replied.ok_or(FeedbackError::FeedbackNotThere)?)
            .await
    }

    /// Rewrites what a piece of feedback says, for whoever wrote it; once submitted, it is marked edited.
    pub async fn rewrite(
        &self,
        feedback: &str,
        comment: Vec<CommentBlock>,
        agent: &Agent,
    ) -> Result<ListedFeedback, FeedbackError> {
        let comment = Comment::new(comment)?;
        let piece = self.found(feedback, agent).await?;
        let rewritten = self
            .carried_out(piece.id, FeedbackCommand::Reword(comment), agent)
            .await?;

        self.listed_one(rewritten.ok_or(FeedbackError::FeedbackNotThere)?)
            .await
    }

    /// Marks a piece of feedback submitted as done, or not done again.
    /// A draft is not there for this: only feedback given is worked through.
    pub async fn mark_done(
        &self,
        feedback: &str,
        done: bool,
        agent: &Agent,
    ) -> Result<ListedFeedback, FeedbackError> {
        let piece = self.found(feedback, agent).await?;
        self.allowed(
            agent,
            Act::Write,
            &piece.project,
            FeedbackError::MarkedByAuthorsAlone,
        )
        .await?;
        let command = if done {
            FeedbackCommand::MarkDone
        } else {
            FeedbackCommand::MarkNotDone
        };
        let marked = self.carried_out(piece.id, command, agent).await?;

        self.listed_one(marked.ok_or(FeedbackError::FeedbackNotThere)?)
            .await
    }

    /// Deletes a piece of feedback, for whoever wrote it, or for an owner of its project once it is given; its words are erased.
    pub async fn delete(&self, feedback: &str, agent: &Agent) -> Result<(), FeedbackError> {
        let piece = self.found(feedback, agent).await?;
        let deleter = if &piece.written_by == agent {
            Deleter::Writer
        } else {
            self.allowed(agent, Act::Manage, &piece.project, FeedbackError::NotYours)
                .await?;
            Deleter::Owner
        };
        self.erased(piece.id, deleter, agent).await
    }

    /// Deletes every piece given or drafted in a deleted reading copy, as the system, a batch at a time,
    /// each erased in a transaction of its own; once none is left, it is done. Asked again, it carries on where it was.
    pub async fn sweep_reading_copy(&self, copy: &ReadingCopyLink) -> Result<(), FeedbackError> {
        self.erased_in_batches(&Subject::ReadingCopy(copy.clone()), Swept::Everything)
            .await
    }

    /// Drops the drafts left unsubmitted in reading copies closed by now, as the system, no more copies than asked;
    /// each copy is kept as done once its drafts are gone. It answers how many copies it did.
    pub async fn drop_drafts_of_closed_copies(&self, at_most: usize) -> Result<u64, FeedbackError> {
        let closed = self
            .copies
            .closed_with_drafts_left(self.clock.now(), at_most)
            .await?;
        for copy in &closed {
            self.erased_in_batches(&Subject::ReadingCopy(copy.clone()), Swept::Drafts)
                .await?;
            transacting::retrying!(&*self.transactions, |transaction| {
                self.copies.drafts_dropped(transaction, copy).await?;

                Ok::<_, FeedbackError>(())
            })?;
        }

        Ok(u64::try_from(closed.len()).unwrap_or(u64::MAX))
    }

    /// Drops every draft a deleted account left, in every copy, as the system, each erased in a transaction of its own.
    /// What it submitted stays, as the authors' to keep, naming nobody. Asked again, it carries on where it was.
    pub async fn drop_drafts_of(&self, account: &AccountId) -> Result<(), FeedbackError> {
        let writer = Agent::Account(account.clone());
        loop {
            let drafts = self.feedback.drafts_of(&Asked::default(), &writer).await?;
            if drafts.is_empty() {
                return Ok(());
            }
            for draft in drafts {
                match self.erased(draft.id, Deleter::System, &Agent::System).await {
                    Ok(()) | Err(FeedbackError::FeedbackNotThere) => {}
                    Err(failure) => return Err(failure),
                }
            }
        }
    }

    /// Erases a copy's pieces as the system, a batch at a time, until none of those swept is left.
    async fn erased_in_batches(
        &self,
        subject: &Subject,
        swept: Swept,
    ) -> Result<(), FeedbackError> {
        loop {
            let batch = match swept {
                Swept::Everything => self.feedback.pieces_on(subject, SWEPT_AT_ONCE).await?,
                Swept::Drafts => self.feedback.drafts_on(subject, SWEPT_AT_ONCE).await?,
            };
            if batch.is_empty() {
                return Ok(());
            }
            for piece in batch {
                match self.erased(piece, Deleter::System, &Agent::System).await {
                    Ok(()) | Err(FeedbackError::FeedbackNotThere) => {}
                    Err(failure) => return Err(failure),
                }
            }
        }
    }

    /// Deletes a piece and erases its words: its events collapse into a snapshot that holds neither what it was about
    /// nor what it said, and the messages already sent about it are forgotten, all in one transaction.
    /// What is given on it, its replies, goes first, by the system, so a piece left behind by a failure still leads to them.
    async fn erased(
        &self,
        piece: FeedbackId,
        deleter: Deleter,
        agent: &Agent,
    ) -> Result<(), FeedbackError> {
        Box::pin(self.erased_in_batches(&Subject::Feedback(piece), Swept::Everything)).await?;

        let aggregate = AggregateId::from(&piece);

        transacting::retrying!(&*self.transactions, |transaction| {
            self.events
                .execute(
                    transaction,
                    &aggregate,
                    FeedbackCommand::Delete(deleter),
                    agent,
                )
                .await?;
            self.catalogue(transaction, piece).await?;
            self.events.compact(transaction, &aggregate, agent).await?;
            self.events
                .forget_published(transaction, &aggregate)
                .await?;

            Ok::<_, FeedbackError>(())
        })
    }
}
