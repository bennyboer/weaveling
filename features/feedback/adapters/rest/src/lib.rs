mod transformer;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing;
use axum::{Extension, Json, Router};
use eventsourcing::Agent;
use feedback_contract::{
    DraftFeedbackRequest, FeedbackCountDTO, FeedbackDTO, PendingDTO, RewriteFeedbackRequest,
    refused, status,
};
use feedback_core::{
    FeedbackCore, FeedbackError, InvalidComment, On, ReadingCopyLink, Status, Subject,
};
use serde::Deserialize;
use serving::Problem;

pub fn router(feedback: FeedbackCore) -> Router {
    Router::new()
        .route("/feedback", routing::post(draft))
        .route("/feedback/drafts", routing::get(drafts_on))
        .route("/feedback/submitted", routing::post(submit_on))
        .route("/feedback/counts", routing::get(counted_on))
        .route("/feedback/on", routing::get(submitted_on))
        .route("/feedback/pending", routing::get(pending_in))
        .route("/feedback/{feedback}", routing::delete(delete))
        .route("/feedback/{feedback}/comment", routing::put(rewrite))
        .route(
            "/feedback/{feedback}/done",
            routing::put(marked_done).delete(marked_not_done),
        )
        .with_state(feedback)
}

/// Where feedback is asked for: some reading copies; or, in every copy, a project, some scenes or some sections;
/// the ids parted by commas, one of them; and, if asked, only what is pending or done.
#[derive(Deserialize)]
struct OnPlaces {
    reading_copies: Option<String>,
    project: Option<String>,
    scenes: Option<String>,
    sections: Option<String>,
    status: Option<String>,
}

impl OnPlaces {
    fn on(&self) -> Result<On, Problem> {
        match (
            &self.reading_copies,
            &self.project,
            &self.scenes,
            &self.sections,
        ) {
            (Some(copies), None, None, None) => Ok(On::Subjects(
                listed::<ReadingCopyLink>(copies)
                    .into_iter()
                    .map(Subject::ReadingCopy)
                    .collect(),
            )),
            (None, Some(project), None, None) => Ok(On::Project(project.clone())),
            (None, None, Some(scenes), None) => Ok(On::Scenes(listed(scenes))),
            (None, None, None, Some(sections)) => Ok(On::Sections(listed(sections))),
            _ => Err(Problem::rejected(
                refused::PLACE_UNNAMED,
                "feedback is asked for on reading copies, in a project, on scenes or on sections, one of them",
            )),
        }
    }

    /// Where drafts are asked for or submitted: the places alone, as a draft is neither pending nor done.
    fn drafts_on(&self) -> Result<On, Problem> {
        if self.status.is_some() {
            return Err(Problem::rejected(
                refused::STATUS_UNKNOWN,
                "a draft is neither pending nor done before it is submitted",
            ));
        }

        self.on()
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

/// Ids parted by commas, each as what it names.
fn listed<T: for<'a> From<&'a str>>(ids: &str) -> Vec<T> {
    ids.split(',')
        .filter(|id| !id.is_empty())
        .map(T::from)
        .collect()
}

async fn submitted_on(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<OnPlaces>,
) -> Result<Json<Vec<FeedbackDTO>>, Problem> {
    let found = feedback
        .submitted(&asked.on()?, asked.status()?, &agent)
        .await
        .map_err(problem_of)?;

    Ok(Json(
        found
            .iter()
            .map(|piece| transformer::to_dto(piece, &agent))
            .collect(),
    ))
}

/// The project whose feedback pending is counted.
#[derive(Deserialize)]
struct InProject {
    project: String,
}

async fn pending_in(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<InProject>,
) -> Result<Json<PendingDTO>, Problem> {
    let pending = feedback
        .pending_in(&asked.project, &agent)
        .await
        .map_err(problem_of)?;

    Ok(Json(transformer::to_pending_dto(&pending)))
}

/// The reading copies whose feedback is counted, their ids parted by commas.
#[derive(Deserialize)]
struct OnReadingCopies {
    reading_copies: String,
}

async fn counted_on(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<OnReadingCopies>,
) -> Result<Json<Vec<FeedbackCountDTO>>, Problem> {
    let subjects = listed::<ReadingCopyLink>(&asked.reading_copies)
        .into_iter()
        .map(Subject::ReadingCopy)
        .collect();
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
    Query(asked): Query<OnPlaces>,
) -> Result<Json<Vec<FeedbackDTO>>, Problem> {
    let found = feedback
        .drafts(&asked.drafts_on()?, &agent)
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
    Query(asked): Query<OnPlaces>,
) -> Result<Json<Vec<FeedbackDTO>>, Problem> {
    let submitted = feedback
        .submit(&asked.drafts_on()?, &agent)
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
    Json(asked): Json<DraftFeedbackRequest>,
) -> Result<(StatusCode, Json<FeedbackDTO>), Problem> {
    let drafted = feedback
        .draft(
            transformer::about_of(asked.about),
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
    Path(id): Path<String>,
    Json(asked): Json<RewriteFeedbackRequest>,
) -> Result<Json<FeedbackDTO>, Problem> {
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
    Path(id): Path<String>,
) -> Result<StatusCode, Problem> {
    feedback.delete(&id, &agent).await.map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

fn problem_of(refusal: FeedbackError) -> Problem {
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
    }
}
