mod transformer;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing;
use axum::{Extension, Json, Router};
use eventsourcing::Agent;
use feedback_contract::{
    DraftFeedbackRequest, FeedbackCountDTO, FeedbackDTO, PendingDTO, RewriteFeedbackRequest, kind,
    refused,
};
use feedback_core::{
    FeedbackCore, FeedbackError, InvalidComment, ReadingCopyLink, SceneLink, SectionLink, Subject,
};
use serde::Deserialize;
use serving::Problem;

pub fn router(feedback: FeedbackCore) -> Router {
    Router::new()
        .route("/feedback", routing::get(feedback_on).post(draft))
        .route("/feedback/submitted", routing::post(submit_on))
        .route("/feedback/counts", routing::get(counted_on))
        .route("/feedback/on", routing::get(given_on))
        .route("/feedback/pending", routing::get(pending_in))
        .route("/feedback/{feedback}", routing::delete(delete))
        .route("/feedback/{feedback}/comment", routing::put(rewrite))
        .route(
            "/feedback/{feedback}/done",
            routing::put(marked_done).delete(marked_not_done),
        )
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
        subject_of(&self.kind, self.id)
    }
}

fn subject_of(kind: &str, id: String) -> Result<Subject, Problem> {
    match kind {
        kind::READING_COPY => Ok(Subject::ReadingCopy(ReadingCopyLink::from(id))),
        unknown => Err(Problem::rejected(
            refused::SUBJECT_UNKNOWN,
            format!("feedback is given on no subject of the kind {unknown}"),
        )),
    }
}

/// The place in the work whose feedback is asked for, in every copy: a scene or a section, one of them.
#[derive(Deserialize)]
struct OnPlace {
    scene: Option<String>,
    section: Option<String>,
}

async fn given_on(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<OnPlace>,
) -> Result<Json<Vec<FeedbackDTO>>, Problem> {
    let found = match (asked.scene, asked.section) {
        (Some(scene), None) => {
            feedback
                .given_on_scene(&SceneLink::from(scene), &agent)
                .await
        }
        (None, Some(section)) => {
            feedback
                .given_on_section(&SectionLink::from(section), &agent)
                .await
        }
        _ => {
            return Err(Problem::rejected(
                refused::PLACE_UNNAMED,
                "feedback is asked for on a scene or on a section, one of them",
            ));
        }
    }
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

/// The subjects whose feedback is counted: their kind, and their ids parted by commas.
#[derive(Deserialize)]
struct OnSubjects {
    kind: String,
    ids: String,
}

async fn counted_on(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<OnSubjects>,
) -> Result<Json<Vec<FeedbackCountDTO>>, Problem> {
    let subjects = asked
        .ids
        .split(',')
        .filter(|id| !id.is_empty())
        .map(|id| subject_of(&asked.kind, id.to_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    let counted = feedback
        .counted_on(subjects, &agent)
        .await
        .map_err(problem_of)?;

    Ok(Json(
        counted.iter().map(transformer::to_count_dto).collect(),
    ))
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

async fn submit_on(
    State(feedback): State<FeedbackCore>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<OnSubject>,
) -> Result<Json<Vec<FeedbackDTO>>, Problem> {
    let submitted = feedback
        .submit_on(&asked.subject()?, &agent)
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
