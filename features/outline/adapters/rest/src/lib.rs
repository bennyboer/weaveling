use axum::Extension;
use axum::Json;
use axum::Router;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, Query, State};
use axum::http::header::ETAG;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing;
use eventsourcing::{Agent, Standing, Version};
use live::{Asked, Broadcaster};
use outline_contract::{
    AddSectionRequest, AddedSectionResponse, AttachRequest, AttachmentDTO, ChangedSectionDTO,
    MoveSectionRequest, OpenOutlineRequest, OutlineChangeDTO, OutlineDTO, PlacedSectionDTO,
    RetitleSectionRequest, refused,
};
use outline_core::{
    Attachment, Changed, InvalidSectionTitle, Outline, OutlineError, OutlineService,
    OutlineServiceError, PlacedSection, SectionId, SectionTitle,
};
use serving::Problem;

pub fn router(outlines: OutlineService) -> Router {
    Router::new()
        .route("/outlines", routing::post(open))
        .route("/outlines/{outline}", routing::get(find))
        .route("/outlines/{outline}/sections", routing::post(add))
        .route(
            "/outlines/{outline}/sections/{section}",
            routing::delete(remove).patch(retitle),
        )
        .route(
            "/outlines/{outline}/sections/{section}/place",
            routing::put(place),
        )
        .route(
            "/outlines/{outline}/sections/{section}/promote",
            routing::post(promote),
        )
        .route(
            "/outlines/{outline}/sections/{section}/demote",
            routing::post(demote),
        )
        .route("/outlines/{outline}/attachments", routing::post(attach))
        .route(
            "/outlines/{outline}/attachments/{kind}/{attached}",
            routing::delete(detach),
        )
        .with_state(outlines)
}

async fn open(
    State(outlines): State<OutlineService>,
    Extension(acting): Extension<Agent>,
    Json(request): Json<OpenOutlineRequest>,
) -> Result<Response, Problem> {
    let opened = outlines
        .open(&request.project, &acting)
        .await
        .map_err(problem_of)?;

    Ok(to_response(
        StatusCode::OK,
        &opened.id.to_string(),
        &opened.standing,
    ))
}

#[derive(Clone)]
struct Following {
    broadcaster: Broadcaster,
    outlines: OutlineService,
}

pub fn live(broadcaster: Broadcaster, outlines: OutlineService) -> Router {
    Router::new()
        .route("/outlines/live", routing::get(follow))
        .with_state(Following {
            broadcaster,
            outlines,
        })
}

async fn follow(
    upgrade: WebSocketUpgrade,
    State(following): State<Following>,
    Extension(acting): Extension<Agent>,
    Query(asked): Query<Asked>,
) -> Response {
    let Some(outline) = asked.get("outline") else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let hearing = following.broadcaster.hear(outline);

    match following.outlines.read(outline, &acting).await {
        Ok(standing) => live::streaming_from(upgrade, standing.version.count(), hearing),
        Err(refused) => problem_of(refused).into_response(),
    }
}

async fn find(
    State(outlines): State<OutlineService>,
    Extension(acting): Extension<Agent>,
    Path(outline): Path<String>,
) -> Result<Response, Problem> {
    let found = outlines.read(&outline, &acting).await.map_err(problem_of)?;

    Ok(to_response(StatusCode::OK, &outline, &found))
}

async fn add(
    State(outlines): State<OutlineService>,
    Extension(acting): Extension<Agent>,
    Path(outline): Path<String>,
    headers: HeaderMap,
    Json(request): Json<AddSectionRequest>,
) -> Result<Response, Problem> {
    let added = outlines
        .add(
            &outline,
            named_section(request.under)?,
            named_section(request.after)?,
            titled(&request.title)?,
            serving::demanded(&headers)?,
            &acting,
        )
        .await
        .map_err(problem_of)?;
    let standing = outlines.get(&outline).await.map_err(problem_of)?;
    let unchanged = Changed {
        version: added.version,
        attachments_changed_in: Vec::new(),
    };

    Ok((
        StatusCode::CREATED,
        [(ETAG, serving::tag(standing.version))],
        Json(AddedSectionResponse {
            section: added.section.to_string(),
            outline: to_change_dto(&outline, &standing, &unchanged),
        }),
    )
        .into_response())
}

async fn retitle(
    State(outlines): State<OutlineService>,
    Extension(acting): Extension<Agent>,
    Path((outline, section)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<RetitleSectionRequest>,
) -> Result<Response, Problem> {
    let changed = outlines
        .retitle(
            &outline,
            section.parse().map_err(invalid_id)?,
            titled(&request.title)?,
            serving::demanded(&headers)?,
            &acting,
        )
        .await
        .map_err(problem_of)?;

    Ok(to_change_response(
        &outline,
        &outlines.get(&outline).await.map_err(problem_of)?,
        &changed,
    ))
}

async fn place(
    State(outlines): State<OutlineService>,
    Extension(acting): Extension<Agent>,
    Path((outline, section)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<MoveSectionRequest>,
) -> Result<Response, Problem> {
    let changed = outlines
        .place(
            &outline,
            section.parse().map_err(invalid_id)?,
            named_section(request.under)?,
            named_section(request.after)?,
            serving::demanded(&headers)?,
            &acting,
        )
        .await
        .map_err(problem_of)?;

    Ok(to_change_response(
        &outline,
        &outlines.get(&outline).await.map_err(problem_of)?,
        &changed,
    ))
}

async fn promote(
    State(outlines): State<OutlineService>,
    Extension(acting): Extension<Agent>,
    Path((outline, section)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let changed = outlines
        .promote(
            &outline,
            section.parse().map_err(invalid_id)?,
            serving::demanded(&headers)?,
            &acting,
        )
        .await
        .map_err(problem_of)?;

    Ok(to_change_response(
        &outline,
        &outlines.get(&outline).await.map_err(problem_of)?,
        &changed,
    ))
}

async fn demote(
    State(outlines): State<OutlineService>,
    Extension(acting): Extension<Agent>,
    Path((outline, section)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let changed = outlines
        .demote(
            &outline,
            section.parse().map_err(invalid_id)?,
            serving::demanded(&headers)?,
            &acting,
        )
        .await
        .map_err(problem_of)?;

    Ok(to_change_response(
        &outline,
        &outlines.get(&outline).await.map_err(problem_of)?,
        &changed,
    ))
}

async fn remove(
    State(outlines): State<OutlineService>,
    Extension(acting): Extension<Agent>,
    Path((outline, section)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let changed = outlines
        .remove(
            &outline,
            section.parse().map_err(invalid_id)?,
            serving::demanded(&headers)?,
            &acting,
        )
        .await
        .map_err(problem_of)?;

    Ok(to_change_response(
        &outline,
        &outlines.get(&outline).await.map_err(problem_of)?,
        &changed,
    ))
}

async fn attach(
    State(outlines): State<OutlineService>,
    Extension(acting): Extension<Agent>,
    Path(outline): Path<String>,
    headers: HeaderMap,
    Json(request): Json<AttachRequest>,
) -> Result<Response, Problem> {
    let changed = outlines
        .attach(
            &outline,
            to_attachment(&request.attachment),
            request.section.parse().map_err(invalid_id)?,
            request.after.as_ref().map(to_attachment),
            serving::demanded(&headers)?,
            &acting,
        )
        .await
        .map_err(problem_of)?;

    Ok(to_change_response(
        &outline,
        &outlines.get(&outline).await.map_err(problem_of)?,
        &changed,
    ))
}

async fn detach(
    State(outlines): State<OutlineService>,
    Extension(acting): Extension<Agent>,
    Path((outline, kind, attached)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let attachment = match kind.as_str() {
        "scenes" => Attachment::scene(&attached),
        "ideas" => Attachment::idea(&attached),
        _ => {
            return Err(Problem::not_found(
                refused::NO_SUCH_KIND,
                "an outline holds scenes and ideas, and nothing else",
            ));
        }
    };

    outlines
        .detach(&outline, attachment, serving::demanded(&headers)?, &acting)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

fn named_section(given: Option<String>) -> Result<Option<SectionId>, Problem> {
    given
        .map(|given| given.parse().map_err(invalid_id))
        .transpose()
}

fn titled(given: &str) -> Result<SectionTitle, Problem> {
    SectionTitle::new(given).map_err(untitled)
}

fn to_response(status: StatusCode, outline: &str, standing: &Standing<Outline>) -> Response {
    (
        status,
        [(ETAG, serving::tag(standing.version))],
        Json(to_dto(outline, &standing.state, standing.version)),
    )
        .into_response()
}

fn to_change_response(outline: &str, standing: &Standing<Outline>, changed: &Changed) -> Response {
    (
        StatusCode::OK,
        [(ETAG, serving::tag(standing.version))],
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

fn untitled(reason: InvalidSectionTitle) -> Problem {
    match reason {
        InvalidSectionTitle::Blank => Problem::unexpected(reason),
        InvalidSectionTitle::ControlCharacter => {
            Problem::unprocessable(refused::TITLE_CONTROL_CHARACTER, reason)
        }
        InvalidSectionTitle::TooLong { max, actual } => {
            Problem::unprocessable(refused::TITLE_TOO_LONG, reason)
                .with("max", max)
                .with("actual", actual)
        }
    }
}

fn invalid_id(reason: ids::InvalidId) -> Problem {
    problem_of(OutlineServiceError::InvalidId(reason))
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
        OutlineServiceError::ProjectNotThere => {
            Problem::not_found(serving::NOT_FOUND, OutlineServiceError::ProjectNotThere)
        }
        OutlineServiceError::NotAnAuthor => {
            Problem::forbidden(refused::NOT_AN_AUTHOR, OutlineServiceError::NotAnAuthor)
        }
        unserveable => Problem::unexpected(unserveable),
    }
}
