use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::header::ETAG;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use eventsourcing::{Agent, ServiceError, Standing, Version};
use outline_contract::{
    AddSectionRequest, AddedSectionResponse, AttachPieceRequest, MoveSectionRequest,
    OpenOutlineRequest, OutlineDTO, PlacedSectionDTO, RetitleSectionRequest,
};
use outline_core::{
    InvalidSectionTitle, Outline, OutlineError, OutlineService, OutlineServiceError, PieceLink,
    PlacedSection, SectionId, SectionTitle,
};
use serving::{Unreadable, demanded, refusal, tag};

pub fn router(outlines: OutlineService) -> Router {
    Router::new()
        .route("/outlines", post(open))
        .route("/outlines/{outline}", get(find))
        .route("/outlines/{outline}/sections", post(add))
        .route(
            "/outlines/{outline}/sections/{section}",
            delete(remove).patch(retitle),
        )
        .route("/outlines/{outline}/sections/{section}/place", put(place))
        .route(
            "/outlines/{outline}/sections/{section}/promote",
            post(promote),
        )
        .route(
            "/outlines/{outline}/sections/{section}/demote",
            post(demote),
        )
        .route("/outlines/{outline}/pieces", post(attach))
        .route("/outlines/{outline}/pieces/{piece}", delete(detach))
        .with_state(outlines)
}

async fn open(
    State(outlines): State<OutlineService>,
    Json(request): Json<OpenOutlineRequest>,
) -> Result<Response, ApiError> {
    let opened = outlines.open(&request.project, &nobody_yet()).await?;

    Ok(reported(
        StatusCode::OK,
        &opened.id.to_string(),
        &opened.standing,
    ))
}

async fn find(
    State(outlines): State<OutlineService>,
    Path(outline): Path<String>,
) -> Result<Response, ApiError> {
    let found = outlines.get(&outline).await?;

    Ok(reported(StatusCode::OK, &outline, &found))
}

async fn add(
    State(outlines): State<OutlineService>,
    Path(outline): Path<String>,
    headers: HeaderMap,
    Json(request): Json<AddSectionRequest>,
) -> Result<Response, ApiError> {
    let added = outlines
        .add(
            &outline,
            named_section(request.under)?,
            named_section(request.after)?,
            titled(&request.title)?,
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;
    let standing = outlines.get(&outline).await?;

    Ok((
        StatusCode::CREATED,
        [(ETAG, tag(standing.version))],
        Json(AddedSectionResponse {
            section: added.section.to_string(),
            outline: to_dto(&outline, &standing.state, standing.version),
        }),
    )
        .into_response())
}

async fn retitle(
    State(outlines): State<OutlineService>,
    Path((outline, section)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<RetitleSectionRequest>,
) -> Result<Response, ApiError> {
    outlines
        .retitle(
            &outline,
            section.parse()?,
            titled(&request.title)?,
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(reported(
        StatusCode::OK,
        &outline,
        &outlines.get(&outline).await?,
    ))
}

async fn place(
    State(outlines): State<OutlineService>,
    Path((outline, section)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<MoveSectionRequest>,
) -> Result<Response, ApiError> {
    outlines
        .place(
            &outline,
            section.parse()?,
            named_section(request.under)?,
            named_section(request.after)?,
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(reported(
        StatusCode::OK,
        &outline,
        &outlines.get(&outline).await?,
    ))
}

async fn promote(
    State(outlines): State<OutlineService>,
    Path((outline, section)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    outlines
        .promote(
            &outline,
            section.parse()?,
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(reported(
        StatusCode::OK,
        &outline,
        &outlines.get(&outline).await?,
    ))
}

async fn demote(
    State(outlines): State<OutlineService>,
    Path((outline, section)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    outlines
        .demote(
            &outline,
            section.parse()?,
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(reported(
        StatusCode::OK,
        &outline,
        &outlines.get(&outline).await?,
    ))
}

async fn remove(
    State(outlines): State<OutlineService>,
    Path((outline, section)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    outlines
        .remove(
            &outline,
            section.parse()?,
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(reported(
        StatusCode::OK,
        &outline,
        &outlines.get(&outline).await?,
    ))
}

async fn attach(
    State(outlines): State<OutlineService>,
    Path(outline): Path<String>,
    headers: HeaderMap,
    Json(request): Json<AttachPieceRequest>,
) -> Result<Response, ApiError> {
    outlines
        .attach(
            &outline,
            PieceLink::from(request.piece.as_str()),
            request.section.parse()?,
            request.after.map(|after| PieceLink::from(after.as_str())),
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(reported(
        StatusCode::OK,
        &outline,
        &outlines.get(&outline).await?,
    ))
}

async fn detach(
    State(outlines): State<OutlineService>,
    Path((outline, piece)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    outlines
        .detach(
            &outline,
            PieceLink::from(piece.as_str()),
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

fn nobody_yet() -> Agent {
    Agent::Anonymous
}

fn named_section(given: Option<String>) -> Result<Option<SectionId>, ApiError> {
    match given {
        None => Ok(None),
        Some(given) => Ok(Some(given.parse()?)),
    }
}

fn titled(given: &str) -> Result<SectionTitle, ApiError> {
    Ok(SectionTitle::new(given)?)
}

fn expected(headers: &HeaderMap) -> Result<Option<Version>, ApiError> {
    Ok(demanded(headers)?)
}

fn reported(status: StatusCode, outline: &str, standing: &Standing<Outline>) -> Response {
    (
        status,
        [(ETAG, tag(standing.version))],
        Json(to_dto(outline, &standing.state, standing.version)),
    )
        .into_response()
}

fn to_dto(outline: &str, held: &Outline, version: Version) -> OutlineDTO {
    OutlineDTO {
        id: outline.to_owned(),
        version: version.count(),
        project: held.project().to_string(),
        sections: held.sections().iter().map(to_section_dto).collect(),
    }
}

fn to_section_dto(placed: &PlacedSection) -> PlacedSectionDTO {
    PlacedSectionDTO {
        section: placed.section.to_string(),
        parent: placed.parent.map(|parent| parent.to_string()),
        title: placed.title.to_string(),
        pieces: placed.pieces.iter().map(|held| held.to_string()).collect(),
    }
}

enum ApiError {
    Unreadable(Unreadable),
    Untitled(InvalidSectionTitle),
    Refused(OutlineServiceError),
}

impl From<OutlineServiceError> for ApiError {
    fn from(error: OutlineServiceError) -> Self {
        Self::Refused(error)
    }
}

impl From<ids::InvalidId> for ApiError {
    fn from(error: ids::InvalidId) -> Self {
        Self::Refused(OutlineServiceError::InvalidId(error))
    }
}

impl From<InvalidSectionTitle> for ApiError {
    fn from(error: InvalidSectionTitle) -> Self {
        Self::Untitled(error)
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
            Self::Untitled(reason) => {
                return (StatusCode::UNPROCESSABLE_ENTITY, reason.to_string()).into_response();
            }
            Self::Refused(refused) => refused,
        };

        let (status, message) = match refused {
            OutlineServiceError::InvalidId(reason) => (StatusCode::BAD_REQUEST, reason.to_string()),
            OutlineServiceError::Events(ServiceError::Refused(OutlineError::NoSuchSection)) => (
                StatusCode::NOT_FOUND,
                OutlineError::NoSuchSection.to_string(),
            ),
            OutlineServiceError::Events(ServiceError::Refused(OutlineError::NotAttached)) => {
                (StatusCode::NOT_FOUND, OutlineError::NotAttached.to_string())
            }
            OutlineServiceError::Events(ServiceError::Refused(OutlineError::AlreadyThere)) => {
                (StatusCode::CONFLICT, OutlineError::AlreadyThere.to_string())
            }
            OutlineServiceError::Events(ServiceError::Refused(
                OutlineError::WouldContainItself,
            )) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                OutlineError::WouldContainItself.to_string(),
            ),
            OutlineServiceError::Events(ServiceError::Refused(OutlineError::NoSuchNeighbour)) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                OutlineError::NoSuchNeighbour.to_string(),
            ),
            OutlineServiceError::Events(events) => refusal(&events),
            unserveable => {
                tracing::error!(error = %unserveable, "an outline request could not be served");

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "something went wrong".to_owned(),
                )
            }
        };

        (status, message).into_response()
    }
}
