mod transformer;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing;
use axum::{Extension, Json, Router};
use eventsourcing::Agent;
use feedback_contract::{DraftFeedbackRequest, FeedbackDTO, RewriteFeedbackRequest, kind, refused};
use feedback_core::{FeedbackCore, FeedbackError, InvalidComment, ReadingCopyLink, Subject};
use serde::Deserialize;
use serving::Problem;

pub fn router(feedback: FeedbackCore) -> Router {
    Router::new()
        .route("/feedback", routing::get(feedback_on).post(draft))
        .route("/feedback/{feedback}", routing::delete(delete))
        .route("/feedback/{feedback}/comment", routing::put(rewrite))
        .with_state(feedback)
}

/// The subject whose feedback is asked for: its kind, as `AboutDTO` names it, and its id.
#[derive(Deserialize)]
struct OnSubject {
    kind: String,
    id: String,
}

impl OnSubject {
    fn subject(self) -> Result<Subject, Problem> {
        match self.kind.as_str() {
            kind::READING_COPY => Ok(Subject::ReadingCopy(ReadingCopyLink::from(self.id))),
            unknown => Err(Problem::rejected(
                refused::SUBJECT_UNKNOWN,
                format!("feedback is given on no subject of the kind {unknown}"),
            )),
        }
    }
}

async fn feedback_on(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<OnSubject>,
) -> Result<Json<Vec<FeedbackDTO>>, Problem> {
    let found = feedback
        .feedback_on(&asked.subject()?, &agent)
        .await
        .map_err(problem_of)?;

    Ok(Json(
        found
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
    }
}
