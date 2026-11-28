mod replies_stub;
mod telling;
mod transformer;

pub use telling::TellingFollowers;

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

use async_trait::async_trait;
use replies_stub::RepliesStub;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing;
use axum::{Extension, Json, Router};
use eventsourcing::Agent;
use feedback_contract::{
    AboutDTO, AskedDTO, DraftFeedbackRequest, FeedbackCountDTO, FeedbackDTO, GiverDTO,
    ListedGiverDTO, PendingDTO, PlaceDTO, RewriteFeedbackRequest, SubjectDTO, UnknownKind, order,
    refused, status,
};
use feedback_core::{
    Asked, FeedbackCore, FeedbackError, InvalidComment, Order, Page, ProjectLink, Status,
};
use live::{About, AccessWatch, Broadcaster, Gate, GatedTopic, Topic};
use serde::Deserialize;
use serving::Problem;

struct ReadersGate(FeedbackCore);

#[async_trait]
impl Gate for ReadersGate {
    async fn admits(
        &self,
        agent: &Agent,
        project: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        Ok(self.0.may_follow(project, agent).await?)
    }

    async fn watch(&self, project: &str) -> AccessWatch {
        self.0.watch_access(project)
    }
}

/// The changes to the feedback given in one project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GivenFeedbackChanges(pub ProjectLink);

impl Display for GivenFeedbackChanges {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

fn given_feedback_changes_of(about: &About) -> Option<GivenFeedbackChanges> {
    about
        .get("project")
        .map(|project| GivenFeedbackChanges(ProjectLink::from(project.as_str())))
}

/// Whoever may read a project follows the feedback given in it under `feedback`, about `{ project }`.
pub fn topic(
    feedback: FeedbackCore,
    broadcaster: Broadcaster<GivenFeedbackChanges>,
) -> Arc<dyn Topic> {
    Arc::new(GatedTopic::new(
        "feedback",
        broadcaster,
        given_feedback_changes_of,
        Arc::new(ReadersGate(feedback)),
    ))
}

pub fn router(feedback: FeedbackCore) -> Router {
    Router::new()
        .route("/feedback", routing::post(draft))
        .route("/feedback/drafts", routing::get(drafts_on))
        .route("/feedback/submitted", routing::post(submit_on))
        .route("/feedback/counts", routing::get(counted_on))
        .route("/feedback/on", routing::get(submitted_on))
        .route("/feedback/pending", routing::get(pending))
        .route("/feedback/givers", routing::get(givers_in))
        .route("/feedback/pieces", routing::get(pieces))
        .route("/feedback/{feedback}", routing::delete(delete))
        .route("/feedback/{feedback}/comment", routing::put(rewrite))
        .route(
            "/feedback/{feedback}/done",
            routing::put(marked_done).delete(marked_not_done),
        )
        .with_state(feedback)
        .layer(Extension(replies_stub::kept()))
}

/// The pieces as listed, each with how many replies it has.
fn with_replies(mut listed: Vec<FeedbackDTO>, replies: &RepliesStub) -> Vec<FeedbackDTO> {
    for piece in &mut listed {
        piece.replies = replies.count(&piece.id);
    }

    listed
}

/// The thread asked for, where `subjects=` names a piece of feedback, whose replies are asked for alone.
fn thread_asked(asked_for: &AskedFor) -> Result<Option<String>, Problem> {
    let subjects = subjects_named(asked_for.subjects.as_deref().unwrap_or_default())?;
    match subjects.as_slice() {
        [SubjectDTO::Feedback(root)] => Ok(Some(root.clone())),
        _ => Ok(None),
    }
}

/// Which feedback is asked for, as `AskedDTO` tells it: `project=`, `subjects=`, `places=` and `given_by=`, each narrowing the others;
/// and, if asked, only what is pending or done, a page at a time.
#[derive(Deserialize)]
struct AskedFor {
    project: Option<String>,
    subjects: Option<String>,
    places: Option<String>,
    given_by: Option<String>,
    status: Option<String>,
    order: Option<String>,
    after: Option<String>,
    limit: Option<usize>,
}

impl AskedFor {
    /// Which feedback is asked for: in a project, on some subjects, at some places in them, given by some, each narrowing the others.
    fn asked(&self) -> Result<Asked, Problem> {
        let asked = AskedDTO {
            project: self.project.clone(),
            subjects: subjects_named(self.subjects.as_deref().unwrap_or_default())?,
            places: PlaceDTO::listed(self.places.as_deref().unwrap_or_default()).map_err(
                |UnknownKind(named)| {
                    Problem::rejected(
                        refused::PLACE_UNKNOWN,
                        format!("a place is a scene or a section, never {named}"),
                    )
                },
            )?,
            given_by: GiverDTO::listed(self.given_by.as_deref().unwrap_or_default()).map_err(
                |UnknownKind(named)| {
                    Problem::rejected(
                        refused::GIVER_UNKNOWN,
                        format!("feedback is given locally or by an account, never by {named}"),
                    )
                },
            )?,
        };
        if !asked.names_anything() {
            return Err(Problem::rejected(
                refused::PLACE_UNNAMED,
                "feedback is asked for in a project, on some subjects, at some places or given by some",
            ));
        }

        transformer::asked_of(asked)
    }

    /// Where what is pending is counted: neither a status nor a page, as it counts what is pending, all of it.
    fn pending_asked(&self) -> Result<Asked, Problem> {
        if self.status.is_some()
            || self.order.is_some()
            || self.after.is_some()
            || self.limit.is_some()
        {
            return Err(Problem::rejected(
                refused::STATUS_UNKNOWN,
                "what is pending is counted whole, neither by status nor a page at a time",
            ));
        }

        self.asked()
    }

    /// Where drafts are asked for or submitted: the places alone, as a draft is neither pending nor done.
    fn drafts_asked(&self) -> Result<Asked, Problem> {
        if self.status.is_some() {
            return Err(Problem::rejected(
                refused::STATUS_UNKNOWN,
                "a draft is neither pending nor done before it is submitted",
            ));
        }

        self.asked()
    }

    /// The page of a listing asked for: oldest first unless asked, after a piece, and at most so many.
    fn page(&self) -> Result<Page, Problem> {
        let order = match self.order.as_deref() {
            None | Some(order::OLDEST) => Order::Oldest,
            Some(order::NEWEST) => Order::Newest,
            Some(unknown) => {
                return Err(Problem::rejected(
                    refused::ORDER_UNKNOWN,
                    format!("feedback is listed oldest or newest first, never {unknown}"),
                ));
            }
        };

        Ok(Page {
            order,
            after: self.after.clone(),
            at_most: self.limit,
        })
    }

    fn status(&self) -> Result<Option<Status>, Problem> {
        match self.status.as_deref() {
            None => Ok(None),
            Some(status::PENDING) => Ok(Some(Status::Pending)),
            Some(status::DONE) => Ok(Some(Status::Done)),
            Some(unknown) => Err(Problem::rejected(
                refused::STATUS_UNKNOWN,
                format!("feedback is pending or done, never {unknown}"),
            )),
        }
    }
}

/// The subjects named, parted by commas, each as its kind and its id.
fn subjects_named(named: &str) -> Result<Vec<SubjectDTO>, Problem> {
    SubjectDTO::listed(named).map_err(|UnknownKind(named)| {
        Problem::rejected(
            refused::SUBJECT_UNKNOWN,
            format!("feedback is given on reading copies, never {named}"),
        )
    })
}

async fn submitted_on(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Extension(replies): Extension<Arc<RepliesStub>>,
    Query(asked_for): Query<AskedFor>,
) -> Result<Json<Vec<FeedbackDTO>>, Problem> {
    if let Some(root) = thread_asked(&asked_for)? {
        return Ok(Json(
            replies
                .on(&feedback, &root, &asked_for.page()?, &agent)
                .await?,
        ));
    }
    let found = feedback
        .submitted(
            &asked_for.asked()?,
            asked_for.status()?,
            &asked_for.page()?,
            &agent,
        )
        .await
        .map_err(problem_of)?;

    Ok(Json(with_replies(
        found
            .iter()
            .map(|piece| transformer::to_dto(piece, &agent))
            .collect(),
        &replies,
    )))
}

/// The pieces asked for by their ids, parted by commas.
#[derive(Deserialize)]
struct ByIds {
    ids: String,
}

async fn pieces(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Extension(replies): Extension<Arc<RepliesStub>>,
    Query(asked): Query<ByIds>,
) -> Result<Json<Vec<FeedbackDTO>>, Problem> {
    let ids: Vec<String> = asked
        .ids
        .split(',')
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .collect();
    let found = feedback.pieces(&ids, &agent).await.map_err(problem_of)?;

    Ok(Json(with_replies(
        found
            .iter()
            .map(|piece| transformer::to_dto(piece, &agent))
            .collect(),
        &replies,
    )))
}

/// The project whose givers are listed.
#[derive(Deserialize)]
struct InProject {
    project: String,
}

async fn pending(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Query(asked_for): Query<AskedFor>,
) -> Result<Json<PendingDTO>, Problem> {
    let pending = feedback
        .pending(&asked_for.pending_asked()?, &agent)
        .await
        .map_err(problem_of)?;

    Ok(Json(transformer::to_pending_dto(&pending)))
}

async fn givers_in(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<InProject>,
) -> Result<Json<Vec<ListedGiverDTO>>, Problem> {
    let givers = feedback
        .givers_in(&asked.project, &agent)
        .await
        .map_err(problem_of)?;

    Ok(Json(
        givers
            .iter()
            .filter_map(|giver| transformer::to_listed_giver_dto(giver, &agent))
            .collect(),
    ))
}

/// The subjects whose feedback is counted, as `subjects=` names them in `AskedDTO`.
#[derive(Deserialize)]
struct OnSubjects {
    subjects: String,
}

async fn counted_on(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<OnSubjects>,
) -> Result<Json<Vec<FeedbackCountDTO>>, Problem> {
    let subjects = subjects_named(&asked.subjects)?
        .into_iter()
        .map(transformer::subject_of)
        .collect::<Result<_, _>>()?;
    let counted = feedback
        .counted_on(subjects, &agent)
        .await
        .map_err(problem_of)?;

    Ok(Json(
        counted.iter().map(transformer::to_count_dto).collect(),
    ))
}

async fn drafts_on(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Query(asked_for): Query<AskedFor>,
) -> Result<Json<Vec<FeedbackDTO>>, Problem> {
    let found = feedback
        .drafts(&asked_for.drafts_asked()?, &agent)
        .await
        .map_err(problem_of)?;

    Ok(Json(
        found
            .iter()
            .map(|piece| transformer::to_dto(piece, &agent))
            .collect(),
    ))
}

async fn submit_on(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Query(asked_for): Query<AskedFor>,
) -> Result<Json<Vec<FeedbackDTO>>, Problem> {
    let submitted = feedback
        .submit(&asked_for.drafts_asked()?, &agent)
        .await
        .map_err(problem_of)?;

    Ok(Json(
        submitted
            .iter()
            .map(|piece| transformer::to_dto(piece, &agent))
            .collect(),
    ))
}

async fn draft(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Extension(replies): Extension<Arc<RepliesStub>>,
    Json(asked): Json<DraftFeedbackRequest>,
) -> Result<(StatusCode, Json<FeedbackDTO>), Problem> {
    if let AboutDTO::Feedback { feedback: to } = &asked.about {
        let replied = replies.reply(&feedback, to, asked.comment, &agent).await?;

        return Ok((StatusCode::CREATED, Json(replied)));
    }
    let drafted = feedback
        .draft(
            transformer::about_of(asked.about)?,
            transformer::comment_of(asked.comment),
            &agent,
        )
        .await
        .map_err(problem_of)?;

    Ok((
        StatusCode::CREATED,
        Json(transformer::to_dto(&drafted, &agent)),
    ))
}

async fn rewrite(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Extension(replies): Extension<Arc<RepliesStub>>,
    Path(id): Path<String>,
    Json(asked): Json<RewriteFeedbackRequest>,
) -> Result<Json<FeedbackDTO>, Problem> {
    if replies.holds(&id) {
        return Ok(Json(replies.rewrite(&id, asked.comment, &agent)?));
    }
    let rewritten = feedback
        .rewrite(&id, transformer::comment_of(asked.comment), &agent)
        .await
        .map_err(problem_of)?;

    Ok(Json(transformer::to_dto(&rewritten, &agent)))
}

async fn marked_done(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Path(id): Path<String>,
) -> Result<Json<FeedbackDTO>, Problem> {
    marked(&feedback, &id, true, &agent).await
}

async fn marked_not_done(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Path(id): Path<String>,
) -> Result<Json<FeedbackDTO>, Problem> {
    marked(&feedback, &id, false, &agent).await
}

async fn marked(
    feedback: &FeedbackCore,
    id: &str,
    done: bool,
    agent: &Agent,
) -> Result<Json<FeedbackDTO>, Problem> {
    let marked = feedback
        .mark_done(id, done, agent)
        .await
        .map_err(problem_of)?;

    Ok(Json(transformer::to_dto(&marked, agent)))
}

async fn delete(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Extension(replies): Extension<Arc<RepliesStub>>,
    Path(id): Path<String>,
) -> Result<StatusCode, Problem> {
    if replies.holds(&id) {
        replies.delete(&id, &agent)?;

        return Ok(StatusCode::NO_CONTENT);
    }
    feedback.delete(&id, &agent).await.map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) fn problem_of(refusal: FeedbackError) -> Problem {
    match refusal {
        FeedbackError::InvalidComment(reason) => match reason {
            InvalidComment::Blank => Problem::rejected(refused::COMMENT_BLANK, reason),
            InvalidComment::ControlCharacter => {
                Problem::rejected(refused::COMMENT_CONTROL_CHARACTER, reason)
            }
            InvalidComment::TooLong { max, actual } => {
                Problem::rejected(refused::COMMENT_TOO_LONG, reason)
                    .with("max", max)
                    .with("actual", actual)
            }
            InvalidComment::LinkUnsafe => Problem::rejected(refused::COMMENT_LINK_UNSAFE, reason),
        },
        FeedbackError::PassageEmpty => Problem::unprocessable(refused::PASSAGE_EMPTY, refusal),
        FeedbackError::FeedbackNotThere => Problem::not_found(serving::NOT_FOUND, refusal),
        FeedbackError::NotYours => Problem::forbidden(refused::NOT_YOURS, refusal),
        FeedbackError::TooManyCounted => Problem::rejected(refused::TOO_MANY_COUNTED, refusal)
            .with("max", feedback_core::COUNTED_AT_ONCE),
        FeedbackError::ReadingCopyClosed => Problem::refused(refused::READING_COPY_CLOSED, refusal),
        FeedbackError::ReplyNotMarked => Problem::refused(refused::REPLY_NOT_MARKED, refusal),
        FeedbackError::RepliesSentAtOnce => Problem::rejected(refused::SUBJECT_UNKNOWN, refusal),
        FeedbackError::ReadingCopyUnknown => {
            Problem::refused(refused::READING_COPY_UNKNOWN, refusal)
        }
        FeedbackError::ProjectNotThere => Problem::not_found(serving::NOT_FOUND, refusal),
        FeedbackError::ProjectUnnamed => Problem::rejected(refused::PLACE_UNNAMED, refusal),
        FeedbackError::MarkedByAuthorsAlone => {
            Problem::forbidden(refused::MARKED_BY_AUTHORS, refusal)
        }
        FeedbackError::Catalog(_)
        | FeedbackError::Transaction(_)
        | FeedbackError::Events(_)
        | FeedbackError::Authorizer(_)
        | FeedbackError::Access(_) => Problem::unexpected(refusal),
    }
}
