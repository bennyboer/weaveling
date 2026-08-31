use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::header::{ETAG, IF_MATCH};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use eventsourcing::{Agent, ServiceError, Standing, StoreError, Version};
use pieces_contract::{AttachPassageRequest, CapturePieceRequest, PieceDTO, RetitlePieceRequest};
use pieces_core::{Piece, PieceService, PieceServiceError, PieceSummary};
use serde::Deserialize;

pub fn router(pieces: PieceService) -> Router {
    Router::new()
        .route("/pieces", get(list).post(capture))
        .route("/pieces/{id}", get(find).patch(retitle).delete(discard))
        .route("/pieces/{id}/passage", put(attach_passage))
        .with_state(pieces)
}

#[derive(Deserialize)]
struct InProject {
    project: String,
}

async fn list(
    State(pieces): State<PieceService>,
    Query(asked): Query<InProject>,
) -> Result<Json<Vec<PieceDTO>>, ApiError> {
    let found = pieces.list(&asked.project).await?;

    Ok(Json(found.iter().map(listed).collect()))
}

fn listed(summary: &PieceSummary) -> PieceDTO {
    PieceDTO {
        id: summary.id.to_string(),
        version: summary.version.count(),
        project: summary.project.to_string(),
        title: summary.title.to_string(),
        passage: summary.passage.as_ref().map(ToString::to_string),
    }
}

async fn capture(
    State(pieces): State<PieceService>,
    Json(request): Json<CapturePieceRequest>,
) -> Result<Response, ApiError> {
    let id = pieces
        .capture(&request.project, &request.title, &nobody_yet())
        .await?
        .to_string();
    let captured = pieces.get(&id).await?;

    Ok(reported(StatusCode::CREATED, &id, &captured))
}

async fn find(
    State(pieces): State<PieceService>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let found = pieces.get(&id).await?;

    Ok(reported(StatusCode::OK, &id, &found))
}

async fn retitle(
    State(pieces): State<PieceService>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RetitlePieceRequest>,
) -> Result<Response, ApiError> {
    pieces
        .retitle(&id, &request.title, expected(&headers)?, &nobody_yet())
        .await?;

    Ok(reported(StatusCode::OK, &id, &pieces.get(&id).await?))
}

async fn attach_passage(
    State(pieces): State<PieceService>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<AttachPassageRequest>,
) -> Result<Response, ApiError> {
    pieces
        .attach_passage(&id, &request.passage, expected(&headers)?, &nobody_yet())
        .await?;

    Ok(reported(StatusCode::OK, &id, &pieces.get(&id).await?))
}

async fn discard(
    State(pieces): State<PieceService>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    pieces
        .discard(&id, expected(&headers)?, &nobody_yet())
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

fn nobody_yet() -> Agent {
    Agent::Anonymous
}

fn expected(headers: &HeaderMap) -> Result<Option<Version>, ApiError> {
    let Some(demanded) = headers.get(IF_MATCH) else {
        return Ok(None);
    };
    let demanded = demanded.to_str().map_err(|_| ApiError::Unreadable)?.trim();

    if demanded == "*" {
        return Ok(None);
    }

    demanded
        .trim_matches('"')
        .parse()
        .map(|counted: u64| Some(Version::of(counted)))
        .map_err(|_| ApiError::Unreadable)
}

fn reported(status: StatusCode, id: &str, standing: &Standing<Piece>) -> Response {
    let tag = format!("\"{}\"", standing.version);

    (
        status,
        [(ETAG, tag)],
        Json(as_dto(id, &standing.state, standing.version)),
    )
        .into_response()
}

fn as_dto(id: &str, piece: &Piece, version: Version) -> PieceDTO {
    PieceDTO {
        id: id.to_owned(),
        version: version.count(),
        project: piece.project().to_string(),
        title: piece.title().to_string(),
        passage: piece.passage().map(ToString::to_string),
    }
}

enum ApiError {
    Unreadable,
    Refused(PieceServiceError),
}

impl From<PieceServiceError> for ApiError {
    fn from(error: PieceServiceError) -> Self {
        Self::Refused(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let refusal = match self {
            Self::Unreadable => {
                return (
                    StatusCode::BAD_REQUEST,
                    "If-Match must be a version, in quotes".to_owned(),
                )
                    .into_response();
            }
            Self::Refused(refusal) => refusal,
        };

        let (status, message) = match refusal {
            PieceServiceError::InvalidId(reason) => (StatusCode::BAD_REQUEST, reason.to_string()),
            PieceServiceError::InvalidTitle(reason) => {
                (StatusCode::BAD_REQUEST, reason.to_string())
            }
            PieceServiceError::Events(ServiceError::NotFound { aggregate, .. }) => (
                StatusCode::NOT_FOUND,
                format!("piece {aggregate} was not found"),
            ),
            PieceServiceError::Events(ServiceError::Refused(refusal)) => {
                (StatusCode::CONFLICT, refusal.to_string())
            }
            PieceServiceError::Events(ServiceError::Store(StoreError::Outdated { .. })) => (
                StatusCode::PRECONDITION_FAILED,
                "this piece has moved on since the version you asked for".to_owned(),
            ),
            unserveable => {
                tracing::error!(error = %unserveable, "a piece request could not be served");

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "something went wrong".to_owned(),
                )
            }
        };

        (status, message).into_response()
    }
}
