use assets_contract::{AssetDTO, OwnerDTO, bound, format, refused};
use assets_core::{
    Asset, AssetError, AssetsCore, AssetsError, Bound, Format, ImageRefused, Limit, Owner,
};
use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, ETAG};
use axum::response::{IntoResponse, Response};
use axum::routing;
use axum::{Extension, Json, Router};
use eventsourcing::{Agent, Standing};
use serde::Deserialize;
use serving::Problem;

const KEPT_FOR_GOOD: &str = "private, max-age=31536000, immutable";

pub fn router(assets: AssetsCore) -> Router {
    let taken = match assets.limits().on_the_upload() {
        Some(limit) => DefaultBodyLimit::max(usize::try_from(limit.most).unwrap_or(usize::MAX)),
        None => DefaultBodyLimit::disable(),
    };

    Router::new()
        .route("/assets", routing::post(upload).layer(taken))
        .route("/assets/{id}", routing::delete(delete))
        .route("/assets/{id}/content", routing::get(content))
        .with_state(assets)
}

#[derive(Deserialize)]
struct IntoProject {
    project: String,
}

#[derive(Deserialize)]
struct AsWide {
    width: Option<u32>,
}

async fn upload(
    State(assets): State<AssetsCore>,
    Extension(acting): Extension<Agent>,
    Query(into): Query<IntoProject>,
    uploaded: Result<Bytes, BytesRejection>,
) -> Result<Response, Problem> {
    let uploaded = uploaded.map_err(|rejection| match assets.limits().on_the_upload() {
        Some(limit) if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            problem_of(AssetsError::LimitReached(limit))
        }
        _ => Problem::rejected(serving::UNREADABLE, rejection),
    })?;
    let (id, uploaded) = assets
        .upload_into(&into.project, uploaded.to_vec(), &acting)
        .await
        .map_err(problem_of)?;

    Ok(to_response(StatusCode::CREATED, &id.to_string(), &uploaded))
}

async fn content(
    State(assets): State<AssetsCore>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
    Query(asked): Query<AsWide>,
) -> Result<Response, Problem> {
    let found = assets
        .content(&id, asked.width, &acting)
        .await
        .map_err(problem_of)?;

    Ok((
        [
            (CONTENT_TYPE, found.content_type),
            (CACHE_CONTROL, KEPT_FOR_GOOD),
        ],
        found.bytes,
    )
        .into_response())
}

async fn delete(
    State(assets): State<AssetsCore>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
) -> Result<StatusCode, Problem> {
    assets.delete(&id, &acting).await.map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

fn to_response(status: StatusCode, id: &str, standing: &Standing<Asset>) -> Response {
    let asset = &standing.state;
    let picture = asset.picture();

    (
        status,
        [(ETAG, serving::tag(standing.version))],
        Json(AssetDTO {
            id: id.to_owned(),
            version: standing.version.count(),
            owner: match asset.owner() {
                Owner::Project(project) => OwnerDTO::Project(project.to_string()),
                Owner::Account(account) => OwnerDTO::Account(account.to_string()),
            },
            format: format_told(picture.format).to_owned(),
            width: picture.width,
            height: picture.height,
            widths: picture.variants.clone(),
            original_size: picture.original_size,
            total_size: asset.total_size(),
        }),
    )
        .into_response()
}

fn format_told(told: Format) -> &'static str {
    match told {
        Format::Jpeg => format::JPEG,
        Format::Png => format::PNG,
        Format::Gif => format::GIF,
        Format::Webp => format::WEBP,
    }
}

fn bound_told(told: Bound) -> &'static str {
    match told {
        Bound::Upload => bound::UPLOAD,
        Bound::Instance => bound::INSTANCE,
        Bound::Account => bound::ACCOUNT,
        Bound::Project => bound::PROJECT,
    }
}

fn problem_of(refusal: AssetsError) -> Problem {
    match refusal {
        AssetsError::InvalidId(reason) => Problem::rejected(refused::ID_INVALID, reason),
        AssetsError::Image(why) => match why {
            ImageRefused::NotAnImage => Problem::unprocessable(refused::NOT_AN_IMAGE, why),
            ImageRefused::Unreadable => Problem::unprocessable(refused::IMAGE_UNREADABLE, why),
            ImageRefused::TooLarge => Problem::unprocessable(refused::IMAGE_TOO_LARGE, why),
        },
        AssetsError::LimitReached(Limit { bound, most, used }) => {
            Problem::too_large(refused::LIMIT_REACHED, refusal)
                .with("bound", bound_told(bound))
                .with("most", most)
                .with("used", used)
        }
        AssetsError::Events(events) => Problem::of(&events, |refusal| match refusal {
            AssetError::Deleted => Problem::not_found(serving::NOT_FOUND, refusal),
            beyond_reach => Problem::unexpected(beyond_reach),
        }),
        AssetsError::ProjectNotThere => {
            Problem::not_found(serving::NOT_FOUND, AssetsError::ProjectNotThere)
        }
        AssetsError::NotAnAuthor => {
            Problem::forbidden(refused::NOT_AN_AUTHOR, AssetsError::NotAnAuthor)
        }
        unserveable => Problem::unexpected(unserveable),
    }
}
