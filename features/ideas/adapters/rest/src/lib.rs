use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::header::ETAG;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use eventsourcing::{Agent, Standing, Version};
use ideas_contract::{CaptureIdeaRequest, IdeaDTO, RetitleIdeaRequest, refused};
use ideas_core::{Idea, IdeaError, IdeaService, IdeaServiceError, IdeaSummary, InvalidIdeaTitle};
use live::{Broadcaster, followed_at};
use serde::Deserialize;
use serving::{Problem, UNREADABLE, Unreadable, demanded, tag};

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
) -> Result<Json<Vec<IdeaDTO>>, ApiError> {
    let found = ideas.list(&asked.project).await?;

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
    Json(request): Json<CaptureIdeaRequest>,
) -> Result<Response, ApiError> {
    let id = ideas
        .capture(&request.project, &request.title, &nobody_yet())
        .await?
        .to_string();
    let captured = ideas.get(&id).await?;

    Ok(to_response(StatusCode::CREATED, &id, &captured))
}

async fn find(
    State(ideas): State<IdeaService>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let found = ideas.get(&id).await?;

    Ok(to_response(StatusCode::OK, &id, &found))
}

async fn retitle(
    State(ideas): State<IdeaService>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RetitleIdeaRequest>,
) -> Result<Response, ApiError> {
    ideas
        .retitle(&id, &request.title, expected(&headers)?, &nobody_yet())
        .await?;

    Ok(to_response(StatusCode::OK, &id, &ideas.get(&id).await?))
}

async fn discard(
    State(ideas): State<IdeaService>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    ideas
        .discard(&id, expected(&headers)?, &nobody_yet())
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

fn nobody_yet() -> Agent {
    Agent::Anonymous
}

fn expected(headers: &HeaderMap) -> Result<Option<Version>, ApiError> {
    Ok(demanded(headers)?)
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

enum ApiError {
    Unreadable(Unreadable),
    Refused(IdeaServiceError),
}

impl From<IdeaServiceError> for ApiError {
    fn from(error: IdeaServiceError) -> Self {
        Self::Refused(error)
    }
}

impl From<Unreadable> for ApiError {
    fn from(error: Unreadable) -> Self {
        Self::Unreadable(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::Unreadable(reason) => Problem::rejected(UNREADABLE, reason),
            Self::Refused(refusal) => problem_of(refusal),
        }
        .into_response()
    }
}

fn problem_of(refusal: IdeaServiceError) -> Problem {
    match refusal {
        IdeaServiceError::InvalidId(reason) => Problem::rejected(refused::ID_INVALID, reason),
        IdeaServiceError::InvalidTitle(reason) => match reason {
            InvalidIdeaTitle::ControlCharacter => {
                Problem::rejected(refused::TITLE_CONTROL_CHARACTER, reason)
            }
            InvalidIdeaTitle::TooLong { max, actual } => {
                Problem::rejected(refused::TITLE_TOO_LONG, reason)
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
