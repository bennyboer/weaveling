use std::sync::{Arc, LazyLock, Mutex};

use eventsourcing::Agent;
use feedback_contract::{AboutDTO, CommentBlockDTO, FeedbackDTO};
use feedback_core::{Comment, FeedbackCore, FeedbackError, FeedbackId, Order, Page};
use serving::Problem;
use time::OffsetDateTime;

use crate::problem_of;
use crate::transformer;

// TODO: replies are kept here, in memory and lost on a restart, until the feedback feature holds them itself
// (M18 replies, R3 to R5). Who may read and write them follows from the piece replied to, as it will;
// but owners may not delete others' replies yet, a deleted piece keeps its replies,
// and nobody following the feedback live is told of them.
/// The replies, one store for every router in the process, as each account's server in a test builds its own.
static KEPT: LazyLock<Arc<RepliesStub>> = LazyLock::new(|| Arc::new(RepliesStub::new()));

pub(crate) fn kept() -> Arc<RepliesStub> {
    KEPT.clone()
}

/// The replies given on feedback: each on a root, a piece given, whose thread is the root and its replies in order.
pub(crate) struct RepliesStub {
    kept: Mutex<Vec<Reply>>,
}

struct Reply {
    id: FeedbackId,
    root: String,
    comment: Comment,
    written_by: Agent,
    name: Option<String>,
    written_at: OffsetDateTime,
    edited: bool,
}

impl Reply {
    fn to_dto(&self, asking: &Agent) -> FeedbackDTO {
        let written_at = clock::text::serialize(self.written_at);

        FeedbackDTO {
            id: self.id.to_string(),
            about: AboutDTO::Feedback {
                feedback: self.root.clone(),
            },
            comment: transformer::to_comment_dto(&self.comment),
            draft: false,
            by_you: &self.written_by == asking,
            written_by: self.name.clone(),
            written_at: written_at.clone(),
            submitted_at: Some(written_at),
            edited: self.edited,
            done: false,
            replies: 0,
        }
    }
}

impl RepliesStub {
    fn new() -> Self {
        Self {
            kept: Mutex::new(Vec::new()),
        }
    }

    /// Replies on a piece given, sent at once. A reply to a reply is one to its root, as threads on a reading copy are flat.
    pub(crate) async fn reply(
        &self,
        feedback: &FeedbackCore,
        to: &str,
        comment: Vec<CommentBlockDTO>,
        agent: &Agent,
    ) -> Result<FeedbackDTO, Problem> {
        let root = self.root_of(to);
        given_root(feedback, &root, agent).await?;
        let comment = Comment::new(transformer::comment_of(comment))
            .map_err(|reason| problem_of(FeedbackError::InvalidComment(reason)))?;
        let name = feedback.name_of(agent).await.map_err(problem_of)?;
        let now = OffsetDateTime::now_utc();
        let reply = Reply {
            id: FeedbackId::generate(now),
            root,
            comment,
            written_by: agent.clone(),
            name,
            written_at: now,
            edited: false,
        };
        let told = reply.to_dto(agent);
        self.kept().push(reply);

        Ok(told)
    }

    /// A thread's replies, in the order asked and paged as asked.
    pub(crate) async fn on(
        &self,
        feedback: &FeedbackCore,
        root: &str,
        page: &Page,
        agent: &Agent,
    ) -> Result<Vec<FeedbackDTO>, Problem> {
        given_root(feedback, root, agent).await?;
        let kept = self.kept();
        let mut thread: Vec<&Reply> = kept.iter().filter(|reply| reply.root == root).collect();
        if page.order == Order::Newest {
            thread.reverse();
        }
        let after = page
            .after
            .as_ref()
            .and_then(|after| {
                thread
                    .iter()
                    .position(|reply| &reply.id.to_string() == after)
            })
            .map_or(0, |at| at + 1);

        Ok(thread
            .into_iter()
            .skip(after)
            .take(page.at_most.unwrap_or(usize::MAX))
            .map(|reply| reply.to_dto(agent))
            .collect())
    }

    pub(crate) fn count(&self, root: &str) -> u32 {
        let counted = self
            .kept()
            .iter()
            .filter(|reply| reply.root == root)
            .count();

        u32::try_from(counted).unwrap_or(u32::MAX)
    }

    pub(crate) fn holds(&self, id: &str) -> bool {
        self.kept().iter().any(|reply| reply.id.to_string() == id)
    }

    /// Rewrites a reply, for whoever wrote it.
    pub(crate) fn rewrite(
        &self,
        id: &str,
        comment: Vec<CommentBlockDTO>,
        agent: &Agent,
    ) -> Result<FeedbackDTO, Problem> {
        let comment = Comment::new(transformer::comment_of(comment))
            .map_err(|reason| problem_of(FeedbackError::InvalidComment(reason)))?;
        let mut kept = self.kept();
        let reply = kept
            .iter_mut()
            .find(|reply| reply.id.to_string() == id)
            .ok_or_else(|| problem_of(FeedbackError::FeedbackNotThere))?;
        if &reply.written_by != agent {
            return Err(problem_of(FeedbackError::NotYours));
        }
        reply.comment = comment;
        reply.edited = true;

        Ok(reply.to_dto(agent))
    }

    /// Deletes a reply, for whoever wrote it.
    pub(crate) fn delete(&self, id: &str, agent: &Agent) -> Result<(), Problem> {
        let mut kept = self.kept();
        let at = kept
            .iter()
            .position(|reply| reply.id.to_string() == id)
            .ok_or_else(|| problem_of(FeedbackError::FeedbackNotThere))?;
        if &kept[at].written_by != agent {
            return Err(problem_of(FeedbackError::NotYours));
        }
        kept.remove(at);

        Ok(())
    }

    fn root_of(&self, to: &str) -> String {
        self.kept()
            .iter()
            .find(|reply| reply.id.to_string() == to)
            .map_or_else(|| to.to_owned(), |reply| reply.root.clone())
    }

    fn kept(&self) -> std::sync::MutexGuard<'_, Vec<Reply>> {
        self.kept.lock().expect("the replies are never poisoned")
    }
}

/// The piece replied to, given and not a draft, as whoever replies finds it; otherwise it is not there for them.
async fn given_root(feedback: &FeedbackCore, root: &str, agent: &Agent) -> Result<(), Problem> {
    let found = feedback
        .pieces(&[root.to_owned()], agent)
        .await
        .map_err(problem_of)?;
    if found.iter().any(|piece| !piece.is_draft()) {
        Ok(())
    } else {
        Err(problem_of(FeedbackError::FeedbackNotThere))
    }
}
