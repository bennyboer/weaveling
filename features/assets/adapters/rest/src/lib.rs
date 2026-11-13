use assets_contract::{
    AccountLimitRequest, AccountStorageDTO, AccountsStorageDTO, AssetDTO, InstanceStorageDTO,
    OwnerDTO, ProjectStorageDTO, ProjectsStorageDTO, UploadDTO, UploadsDTO, UsageDTO, bound,
    format, refused, sort,
};
use assets_core::{
    Asset, AssetError, AssetsCore, AssetsError, Bound, Format, Held, ImageRefused, Limit, Listed,
    Owner, Sorted, UploadMark, UploadsSorted,
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
use time::OffsetDateTime;

const KEPT_FOR_GOOD: &str = "private, max-age=31536000, immutable";
const A_PAGE: u64 = 50;

pub fn router(assets: AssetsCore) -> Router {
    let taken = match assets.limits().on_the_upload() {
        Some(limit) => DefaultBodyLimit::max(usize::try_from(limit.most).unwrap_or(usize::MAX)),
        None => DefaultBodyLimit::disable(),
    };

    Router::new()
        .route("/assets", routing::post(upload).layer(taken).get(uploads))
        .route("/assets/usage", routing::get(usage))
        .route("/assets/storage", routing::get(instance_storage))
        .route("/assets/storage/accounts", routing::get(accounts_storage))
        .route("/assets/storage/projects", routing::get(projects_storage))
        .route("/assets/{id}", routing::delete(delete))
        .route("/assets/{id}/content", routing::get(content))
        .route(
            "/assets/accounts/{account}/limit",
            routing::put(give_account_limit).delete(take_back_account_limit),
        )
        .with_state(assets)
}

#[derive(Deserialize)]
struct IntoProject {
    project: Option<String>,
}

#[derive(Deserialize)]
struct Paging {
    sort: Option<String>,
    after: Option<String>,
    search: Option<String>,
}

impl Paging {
    fn read(&self) -> Result<(Sorted, Option<Held>), Problem> {
        let sorted = match self.sort.as_deref() {
            None | Some(sort::MOST_USED) => Sorted::MostUsed,
            Some(sort::ID) => Sorted::ById,
            Some(unknown) => {
                return Err(Problem::rejected(
                    refused::SORT_UNKNOWN,
                    format!("no sort {unknown}; take most-used or id"),
                ));
            }
        };
        let after = self
            .after
            .as_deref()
            .map(|cursor| {
                cursor
                    .split_once('.')
                    .and_then(|(total, id)| {
                        Some(Held {
                            id: id.to_owned(),
                            total: total.parse().ok()?,
                            name: None,
                            email: None,
                        })
                    })
                    .filter(|held| !held.id.is_empty())
                    .ok_or_else(|| {
                        Problem::rejected(refused::CURSOR_INVALID, "no page goes on from there")
                    })
            })
            .transpose()?;

        Ok((sorted, after))
    }
}

#[derive(Deserialize)]
struct UploadsPaging {
    sort: Option<String>,
    after: Option<String>,
}

impl UploadsPaging {
    fn read(&self) -> Result<(UploadsSorted, Option<UploadMark>), Problem> {
        let sorted = match self.sort.as_deref() {
            None | Some(sort::NEWEST) => UploadsSorted::Newest,
            Some(sort::LARGEST) => UploadsSorted::Largest,
            Some(unknown) => {
                return Err(Problem::rejected(
                    refused::SORT_UNKNOWN,
                    format!("no sort {unknown}; take newest or largest"),
                ));
            }
        };
        let after = self
            .after
            .as_deref()
            .map(|cursor| {
                mark_read(cursor).ok_or_else(|| {
                    Problem::rejected(refused::CURSOR_INVALID, "no page goes on from there")
                })
            })
            .transpose()?;

        Ok((sorted, after))
    }
}

/// The upload a page goes on after, read from its cursor: what it takes, when it was uploaded in nanoseconds, and its id.
fn mark_read(cursor: &str) -> Option<UploadMark> {
    let mut parts = cursor.splitn(3, '.');
    let total_size = parts.next()?.parse().ok()?;
    let uploaded_at =
        OffsetDateTime::from_unix_timestamp_nanos(parts.next()?.parse().ok()?).ok()?;
    let id = parts.next()?.parse().ok()?;

    Some(UploadMark {
        id,
        total_size,
        uploaded_at,
    })
}

fn mark_written(mark: &UploadMark) -> String {
    format!(
        "{}.{}.{}",
        mark.total_size,
        mark.uploaded_at.unix_timestamp_nanos(),
        mark.id
    )
}

fn next_of<T>(listed: &Listed<T>) -> Option<String> {
    listed
        .next
        .as_ref()
        .map(|held| format!("{}.{}", held.total, held.id))
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
    let (id, uploaded) = match into.project {
        Some(project) => {
            assets
                .upload_into(&project, uploaded.to_vec(), &acting)
                .await
        }
        None => assets.upload_as_own(uploaded.to_vec(), &acting).await,
    }
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

async fn usage(
    State(assets): State<AssetsCore>,
    Extension(acting): Extension<Agent>,
) -> Result<Json<UsageDTO>, Problem> {
    let own = assets.usage_of(&acting).await.map_err(problem_of)?;

    Ok(Json(UsageDTO {
        used: own.used,
        most: own.most,
    }))
}

async fn give_account_limit(
    State(assets): State<AssetsCore>,
    Extension(acting): Extension<Agent>,
    Path(account): Path<String>,
    Json(request): Json<AccountLimitRequest>,
) -> Result<StatusCode, Problem> {
    assets
        .give_account_limit(&account, request.most, &acting)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn take_back_account_limit(
    State(assets): State<AssetsCore>,
    Extension(acting): Extension<Agent>,
    Path(account): Path<String>,
) -> Result<StatusCode, Problem> {
    assets
        .take_back_account_limit(&account, &acting)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn instance_storage(
    State(assets): State<AssetsCore>,
    Extension(acting): Extension<Agent>,
) -> Result<Json<InstanceStorageDTO>, Problem> {
    let storage = assets.instance_storage(&acting).await.map_err(problem_of)?;

    Ok(Json(InstanceStorageDTO {
        used: storage.used,
        most: storage.most,
    }))
}

async fn accounts_storage(
    State(assets): State<AssetsCore>,
    Extension(acting): Extension<Agent>,
    Query(paging): Query<Paging>,
) -> Result<Json<AccountsStorageDTO>, Problem> {
    let (sorted, after) = paging.read()?;
    let listed = assets
        .accounts_storage(
            sorted,
            after.as_ref(),
            paging.search.as_deref(),
            A_PAGE,
            &acting,
        )
        .await
        .map_err(problem_of)?;

    Ok(Json(AccountsStorageDTO {
        next: next_of(&listed),
        accounts: listed
            .listed
            .into_iter()
            .map(|account| AccountStorageDTO {
                account: account.account.to_string(),
                name: account.name,
                email: account.email,
                used: account.used,
                most: account.most,
                own_limit: account.own_limit,
            })
            .collect(),
    }))
}

async fn projects_storage(
    State(assets): State<AssetsCore>,
    Extension(acting): Extension<Agent>,
    Query(paging): Query<Paging>,
) -> Result<Json<ProjectsStorageDTO>, Problem> {
    let (sorted, after) = paging.read()?;
    let listed = assets
        .projects_storage(
            sorted,
            after.as_ref(),
            paging.search.as_deref(),
            A_PAGE,
            &acting,
        )
        .await
        .map_err(problem_of)?;

    Ok(Json(ProjectsStorageDTO {
        next: next_of(&listed),
        projects: listed
            .listed
            .into_iter()
            .map(|project| ProjectStorageDTO {
                project: project.project.to_string(),
                name: project.name,
                used: project.used,
                most: project.most,
                owners: project.owners.iter().map(ToString::to_string).collect(),
            })
            .collect(),
    }))
}

async fn uploads(
    State(assets): State<AssetsCore>,
    Extension(acting): Extension<Agent>,
    Query(paging): Query<UploadsPaging>,
) -> Result<Json<UploadsDTO>, Problem> {
    let (sorted, after) = paging.read()?;
    let listed = assets
        .uploads_of(sorted, after.as_ref(), A_PAGE, &acting)
        .await
        .map_err(problem_of)?;

    Ok(Json(UploadsDTO {
        next: listed.next.as_ref().map(mark_written),
        uploads: listed
            .listed
            .into_iter()
            .map(|own| UploadDTO {
                id: own.upload.id.to_string(),
                owner: owner_told(&own.upload.owner),
                project_name: own.upload.project_name,
                format: format_told(own.upload.format).to_owned(),
                width: own.upload.width,
                height: own.upload.height,
                total_size: own.upload.total_size,
                uploaded_at: clock::text::serialize(own.upload.uploaded_at),
                deletable: own.deletable,
            })
            .collect(),
    }))
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
            owner: owner_told(asset.owner()),
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

fn owner_told(owner: &Owner) -> OwnerDTO {
    match owner {
        Owner::Project(project) => OwnerDTO::Project(project.to_string()),
        Owner::Account(account) => OwnerDTO::Account(account.to_string()),
    }
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
        AssetsError::NoAccount => Problem::forbidden(refused::NO_ACCOUNT, AssetsError::NoAccount),
        AssetsError::NotAnOperator => {
            Problem::forbidden(refused::NOT_AN_OPERATOR, AssetsError::NotAnOperator)
        }
        AssetsError::LimitUncountable => {
            Problem::unprocessable(refused::LIMIT_UNCOUNTABLE, AssetsError::LimitUncountable)
        }
        unserveable => Problem::unexpected(unserveable),
    }
}
