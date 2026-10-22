use axum::Extension;
use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::header::ETAG;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use eventsourcing::{Agent, Standing};
use ideas_contract::{CaptureIdeaRequest, IdeaDTO, RetitleIdeaRequest, refused};
use ideas_core::{Idea, IdeaError, IdeaService, IdeaServiceError, IdeaSummary, InvalidIdeaTitle};
use live::{Broadcaster, followed_at};
use serde::Deserialize;
use serving::{Problem, demanded, tag};

pub fn live(broadcaster: Broadcaster) -> Router {
    followed_at("/ideas/live", broadcaster, |asked| {
        asked.get("project").cloned()
    })
}

pub fn router(ideas: IdeaService) -> Router {
    Router::new()
        .route("/ideas", get(list).post(capture))
        .route("/ideas/{id}", get(find).patch(retitle).delete(discard))
        .with_state(ideas)
}

#[derive(Deserialize)]
struct InProject {
    project: String,
}

async fn list(
    State(ideas): State<IdeaService>,
    Query(asked): Query<InProject>,
) -> Result<Json<Vec<IdeaDTO>>, Problem> {
    let found = ideas.list(&asked.project).await.map_err(problem_of)?;

    Ok(Json(found.iter().map(to_dto).collect()))
}

fn to_dto(summary: &IdeaSummary) -> IdeaDTO {
    IdeaDTO {
        id: summary.id.to_string(),
        version: summary.version.count(),
        project: summary.project.to_string(),
        title: summary.title.to_string(),
    }
}

async fn capture(
    State(ideas): State<IdeaService>,
    Extension(acting): Extension<Agent>,
    Json(request): Json<CaptureIdeaRequest>,
) -> Result<Response, Problem> {
    let id = ideas
        .capture(&request.project, &request.title, &acting)
        .await
        .map_err(problem_of)?
        .to_string();
    let captured = ideas.get(&id).await.map_err(problem_of)?;

    Ok(to_response(StatusCode::CREATED, &id, &captured))
}

async fn find(
    State(ideas): State<IdeaService>,
    Path(id): Path<String>,
) -> Result<Response, Problem> {
    let found = ideas.get(&id).await.map_err(problem_of)?;

    Ok(to_response(StatusCode::OK, &id, &found))
}

async fn retitle(
    State(ideas): State<IdeaService>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RetitleIdeaRequest>,
) -> Result<Response, Problem> {
    ideas
        .retitle(&id, &request.title, demanded(&headers)?, &acting)
        .await
        .map_err(problem_of)?;

    Ok(to_response(
        StatusCode::OK,
        &id,
        &ideas.get(&id).await.map_err(problem_of)?,
    ))
}

async fn discard(
    State(ideas): State<IdeaService>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    ideas
        .discard(&id, demanded(&headers)?, &acting)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

fn to_response(status: StatusCode, id: &str, standing: &Standing<Idea>) -> Response {
    let idea = &standing.state;

    (
        status,
        [(ETAG, tag(standing.version))],
        Json(IdeaDTO {
            id: id.to_owned(),
            version: standing.version.count(),
            project: idea.project().to_string(),
            title: idea.title().to_string(),
        }),
    )
        .into_response()
}

fn problem_of(refusal: IdeaServiceError) -> Problem {
    match refusal {
        IdeaServiceError::InvalidId(reason) => Problem::rejected(refused::ID_INVALID, reason),
        IdeaServiceError::InvalidTitle(reason) => match reason {
            InvalidIdeaTitle::Blank => Problem::unexpected(reason),
            InvalidIdeaTitle::ControlCharacter => {
                Problem::unprocessable(refused::TITLE_CONTROL_CHARACTER, reason)
            }
            InvalidIdeaTitle::TooLong { max, actual } => {
                Problem::unprocessable(refused::TITLE_TOO_LONG, reason)
                    .with("max", max)
                    .with("actual", actual)
            }
        },
        IdeaServiceError::Events(events) => Problem::of(&events, |refusal| match refusal {
            IdeaError::Discarded => Problem::refused(refused::DISCARDED, refusal),
            beyond_reach => Problem::unexpected(beyond_reach),
        }),
        unserveable => Problem::unexpected(unserveable),
    }
}
