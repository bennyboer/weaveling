use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::header::ETAG;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use eventsourcing::{Agent, Standing, Version};
use ideas_contract::{CaptureIdeaRequest, IdeaDTO, RetitleIdeaRequest};
use ideas_core::{Idea, IdeaService, IdeaServiceError, IdeaSummary};
use serde::Deserialize;
use serving::{Unreadable, demanded, refusal, tag};

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
        let refused = match self {
            Self::Unreadable(reason) => {
                return (StatusCode::BAD_REQUEST, reason.to_string()).into_response();
            }
            Self::Refused(refused) => refused,
        };

        let (status, message) = match refused {
            IdeaServiceError::InvalidId(reason) => (StatusCode::BAD_REQUEST, reason.to_string()),
            IdeaServiceError::InvalidTitle(reason) => (StatusCode::BAD_REQUEST, reason.to_string()),
            IdeaServiceError::Events(events) => refusal(&events),
            unserveable => {
                tracing::error!(error = %unserveable, "an idea request could not be served");

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "something went wrong".to_owned(),
                )
            }
        };

        (status, message).into_response()
    }
}
