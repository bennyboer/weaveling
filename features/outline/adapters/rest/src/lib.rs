use axum::Json;
use axum::Router;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, Query, State};
use axum::http::header::ETAG;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use eventsourcing::{Agent, Standing, Version};
use live::{Broadcaster, streaming, streaming_from};
use outline_contract::{
    AddSectionRequest, AddedSectionResponse, AttachRequest, AttachmentDTO, ChangedSectionDTO,
    MoveSectionRequest, OpenOutlineRequest, OutlineChangeDTO, OutlineDTO, PlacedSectionDTO,
    RetitleSectionRequest, refused,
};
use outline_core::{
    Attachment, Changed, InvalidSectionTitle, Outline, OutlineError, OutlineService,
    OutlineServiceError, PlacedSection, SectionId, SectionTitle,
};
use serde::Deserialize;
use serving::{Problem, UNREADABLE, Unreadable, demanded, tag};

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
        .route("/outlines/{outline}/attachments", post(attach))
        .route(
            "/outlines/{outline}/attachments/{kind}/{attached}",
            delete(detach),
        )
        .with_state(outlines)
}

async fn open(
    State(outlines): State<OutlineService>,
    Json(request): Json<OpenOutlineRequest>,
) -> Result<Response, ApiError> {
    let opened = outlines.open(&request.project, &nobody_yet()).await?;

    Ok(to_response(
        StatusCode::OK,
        &opened.id.to_string(),
        &opened.standing,
    ))
}

#[derive(Clone)]
struct Live {
    broadcaster: Broadcaster,
    outlines: OutlineService,
}

pub fn live(broadcaster: Broadcaster, outlines: OutlineService) -> Router {
    Router::new()
        .route("/outlines/live", get(follow))
        .with_state(Live {
            broadcaster,
            outlines,
        })
}

async fn follow(
    upgrade: WebSocketUpgrade,
    State(live): State<Live>,
    Query(followed): Query<Followed>,
) -> Response {
    let hearing = live.broadcaster.hear(&followed.outline);

    match live.outlines.get(&followed.outline).await {
        Ok(standing) => streaming_from(upgrade, standing.version.count(), hearing),
        Err(_) => streaming(upgrade, hearing),
    }
}

#[derive(Deserialize)]
struct Followed {
    outline: String,
}

async fn find(
    State(outlines): State<OutlineService>,
    Path(outline): Path<String>,
) -> Result<Response, ApiError> {
    let found = outlines.get(&outline).await?;

    Ok(to_response(StatusCode::OK, &outline, &found))
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
    let unchanged = Changed {
        version: added.version,
        attachments_changed_in: Vec::new(),
    };

    Ok((
        StatusCode::CREATED,
        [(ETAG, tag(standing.version))],
        Json(AddedSectionResponse {
            section: added.section.to_string(),
            outline: to_change_dto(&outline, &standing, &unchanged),
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
    let changed = outlines
        .retitle(
            &outline,
            section.parse()?,
            titled(&request.title)?,
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(to_change_response(
        &outline,
        &outlines.get(&outline).await?,
        &changed,
    ))
}

async fn place(
    State(outlines): State<OutlineService>,
    Path((outline, section)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<MoveSectionRequest>,
) -> Result<Response, ApiError> {
    let changed = outlines
        .place(
            &outline,
            section.parse()?,
            named_section(request.under)?,
            named_section(request.after)?,
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(to_change_response(
        &outline,
        &outlines.get(&outline).await?,
        &changed,
    ))
}

async fn promote(
    State(outlines): State<OutlineService>,
    Path((outline, section)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let changed = outlines
        .promote(
            &outline,
            section.parse()?,
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(to_change_response(
        &outline,
        &outlines.get(&outline).await?,
        &changed,
    ))
}

async fn demote(
    State(outlines): State<OutlineService>,
    Path((outline, section)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let changed = outlines
        .demote(
            &outline,
            section.parse()?,
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(to_change_response(
        &outline,
        &outlines.get(&outline).await?,
        &changed,
    ))
}

async fn remove(
    State(outlines): State<OutlineService>,
    Path((outline, section)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let changed = outlines
        .remove(
            &outline,
            section.parse()?,
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(to_change_response(
        &outline,
        &outlines.get(&outline).await?,
        &changed,
    ))
}

async fn attach(
    State(outlines): State<OutlineService>,
    Path(outline): Path<String>,
    headers: HeaderMap,
    Json(request): Json<AttachRequest>,
) -> Result<Response, ApiError> {
    let changed = outlines
        .attach(
            &outline,
            to_attachment(&request.attachment),
            request.section.parse()?,
            request.after.as_ref().map(to_attachment),
            expected(&headers)?,
            &nobody_yet(),
        )
        .await?;

    Ok(to_change_response(
        &outline,
        &outlines.get(&outline).await?,
        &changed,
    ))
}

async fn detach(
    State(outlines): State<OutlineService>,
    Path((outline, kind, attached)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let attachment = match kind.as_str() {
        "scenes" => Attachment::scene(&attached),
        "ideas" => Attachment::idea(&attached),
        _ => return Err(ApiError::NoSuchKind),
    };

    outlines
        .detach(&outline, attachment, expected(&headers)?, &nobody_yet())
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

fn to_response(status: StatusCode, outline: &str, standing: &Standing<Outline>) -> Response {
    (
        status,
        [(ETAG, tag(standing.version))],
        Json(to_dto(outline, &standing.state, standing.version)),
    )
        .into_response()
}

fn to_change_response(outline: &str, standing: &Standing<Outline>, changed: &Changed) -> Response {
    (
        StatusCode::OK,
        [(ETAG, tag(standing.version))],
        Json(to_change_dto(outline, standing, changed)),
    )
        .into_response()
}

fn to_change_dto(
    outline: &str,
    standing: &Standing<Outline>,
    changed: &Changed,
) -> OutlineChangeDTO {
    let nothing_else_landed = standing.version == changed.version;

    OutlineChangeDTO {
        id: outline.to_owned(),
        version: standing.version.count(),
        project: standing.state.project().to_string(),
        sections: standing
            .state
            .sections()
            .iter()
            .map(|placed| ChangedSectionDTO {
                section: placed.section.to_string(),
                parent: placed.parent.map(|parent| parent.to_string()),
                title: placed.title.to_string(),
                attachments: (!nothing_else_landed
                    || changed.attachments_changed_in.contains(&placed.section))
                .then(|| placed.attachments.iter().map(to_attachment_dto).collect()),
            })
            .collect(),
    }
}

fn to_dto(outline: &str, held: &Outline, version: Version) -> OutlineDTO {
    OutlineDTO {
        id: outline.to_owned(),
        version: version.count(),
        project: held.project().to_string(),
        sections: held.sections().iter().map(to_section_dto).collect(),
    }
}

fn to_attachment(dto: &AttachmentDTO) -> Attachment {
    match dto {
        AttachmentDTO::Scene { id } => Attachment::scene(id),
        AttachmentDTO::Idea { id } => Attachment::idea(id),
    }
}

fn to_attachment_dto(attachment: &Attachment) -> AttachmentDTO {
    match attachment {
        Attachment::Scene(scene) => AttachmentDTO::Scene {
            id: scene.to_string(),
        },
        Attachment::Idea(idea) => AttachmentDTO::Idea {
            id: idea.to_string(),
        },
    }
}

fn to_section_dto(placed: &PlacedSection) -> PlacedSectionDTO {
    PlacedSectionDTO {
        section: placed.section.to_string(),
        parent: placed.parent.map(|parent| parent.to_string()),
        title: placed.title.to_string(),
        attachments: placed.attachments.iter().map(to_attachment_dto).collect(),
    }
}

enum ApiError {
    Unreadable(Unreadable),
    Untitled(InvalidSectionTitle),
    NoSuchKind,
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
        match self {
            Self::Unreadable(reason) => Problem::rejected(UNREADABLE, reason),
            Self::Untitled(reason) => match reason {
                InvalidSectionTitle::ControlCharacter => {
                    Problem::unprocessable(refused::TITLE_CONTROL_CHARACTER, reason)
                }
                InvalidSectionTitle::TooLong { max, actual } => {
                    Problem::unprocessable(refused::TITLE_TOO_LONG, reason)
                        .with("max", max)
                        .with("actual", actual)
                }
            },
            Self::NoSuchKind => Problem::not_found(
                refused::NO_SUCH_KIND,
                "an outline holds scenes and ideas, and nothing else",
            ),
            Self::Refused(refusal) => problem_of(refusal),
        }
        .into_response()
    }
}

fn problem_of(refusal: OutlineServiceError) -> Problem {
    match refusal {
        OutlineServiceError::InvalidId(reason) => Problem::rejected(refused::ID_INVALID, reason),
        OutlineServiceError::Events(events) => Problem::of(&events, |refusal| match refusal {
            OutlineError::NoSuchSection => Problem::not_found(refused::NO_SUCH_SECTION, refusal),
            OutlineError::WouldContainItself => {
                Problem::unprocessable(refused::WOULD_CONTAIN_ITSELF, refusal)
            }
            OutlineError::NoSuchNeighbour => {
                Problem::unprocessable(refused::NO_SUCH_NEIGHBOUR, refusal)
            }
            OutlineError::NotAttached => Problem::not_found(refused::NOT_ATTACHED, refusal),
            OutlineError::Discarded => Problem::refused(refused::DISCARDED, refusal),
            beyond_reach => Problem::unexpected(beyond_reach),
        }),
        unserveable => Problem::unexpected(unserveable),
    }
}
