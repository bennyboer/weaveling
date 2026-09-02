use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::header::ETAG;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use eventsourcing::{Agent, Standing, Version};
use pieces_contract::{AttachPassageRequest, CapturePieceRequest, PieceDTO, RetitlePieceRequest};
use pieces_core::{Piece, PieceService, PieceServiceError, PieceSummary};
use serde::Deserialize;
use serving::{Unreadable, demanded, refusal, tag};

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
    Ok(demanded(headers)?)
}

fn reported(status: StatusCode, id: &str, standing: &Standing<Piece>) -> Response {
    (
        status,
        [(ETAG, tag(standing.version))],
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
    Unreadable(Unreadable),
    Refused(PieceServiceError),
}

impl From<PieceServiceError> for ApiError {
    fn from(error: PieceServiceError) -> Self {
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
            PieceServiceError::InvalidId(reason) => (StatusCode::BAD_REQUEST, reason.to_string()),
            PieceServiceError::InvalidTitle(reason) => {
                (StatusCode::BAD_REQUEST, reason.to_string())
            }
            PieceServiceError::Events(events) => refusal(&events),
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
