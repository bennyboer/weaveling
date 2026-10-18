use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{FromRef, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use scenes_contract::{
    CreateSceneRequest, LinkIdeaRequest, QuotationRestylingDTO, RetitleSceneRequest, SceneDTO,
    ScenePageDTO, SceneSummaryDTO,
};
use scenes_core::{
    CatalogError, ProjectLink, QuotationRestylingTaskError, QuotationRestylingTasks, Scene,
    SceneCatalog, SceneId, SceneService, SceneServiceError, SceneSummary, StoreError,
};
use serde::Deserialize;

#[derive(Clone)]
struct Scenes {
    service: SceneService,
    catalog: Arc<dyn SceneCatalog>,
}

impl FromRef<Scenes> for SceneService {
    fn from_ref(scenes: &Scenes) -> Self {
        scenes.service.clone()
    }
}

impl FromRef<Scenes> for Arc<dyn SceneCatalog> {
    fn from_ref(scenes: &Scenes) -> Self {
        scenes.catalog.clone()
    }
}

pub fn router(service: SceneService, catalog: Arc<dyn SceneCatalog>) -> Router {
    Router::new()
        .route("/scenes", get(list).post(create))
        .route("/scenes/{id}", get(find).patch(retitle).delete(remove))
        .route("/scenes/{id}/ideas", post(link))
        .route("/scenes/{id}/ideas/{idea}", delete(unlink))
        .with_state(Scenes { service, catalog })
}

pub fn quotation_restylings(tasks: Arc<dyn QuotationRestylingTasks>) -> Router {
    Router::new()
        .route("/scenes/quotation-restylings/{project}", get(progress))
        .with_state(tasks)
}

async fn progress(
    State(tasks): State<Arc<dyn QuotationRestylingTasks>>,
    Path(project): Path<String>,
) -> Result<Json<QuotationRestylingDTO>, ApiError> {
    let progress = tasks
        .progress(&ProjectLink::from(project.as_str()))
        .await?
        .ok_or(ApiError::NeverRestyled(project))?;

    Ok(Json(QuotationRestylingDTO {
        change: progress.change,
        total: progress.total,
        done: progress.done.min(progress.total),
        finished: progress.finished,
    }))
}

pub const PAGE: usize = 1000;

#[derive(Deserialize)]
struct InProject {
    project: String,
    after: Option<String>,
}

async fn list(
    State(catalog): State<Arc<dyn SceneCatalog>>,
    Query(asked): Query<InProject>,
) -> Result<Json<ScenePageDTO>, ApiError> {
    let after = asked
        .after
        .as_deref()
        .map(str::parse::<SceneId>)
        .transpose()
        .map_err(SceneServiceError::from)?;
    let mut found = catalog
        .in_project_after(&ProjectLink::from(asked.project), after, PAGE + 1)
        .await?;
    let more = found.len() > PAGE;
    found.truncate(PAGE);

    Ok(Json(ScenePageDTO {
        next: more
            .then(|| found.last().map(|last| last.id.to_string()))
            .flatten(),
        scenes: found.iter().map(to_summary_dto).collect(),
    }))
}

fn to_summary_dto(summary: &SceneSummary) -> SceneSummaryDTO {
    SceneSummaryDTO {
        id: summary.id.to_string(),
        project: summary.project.to_string(),
        title: summary.title.to_string(),
        opening: summary.opening.clone(),
    }
}

async fn create(
    State(scenes): State<SceneService>,
    Json(request): Json<CreateSceneRequest>,
) -> Result<(StatusCode, Json<SceneDTO>), ApiError> {
    let created = scenes.create(&request.project).await?;

    Ok((StatusCode::CREATED, Json(to_dto(&created))))
}

async fn find(
    State(scenes): State<SceneService>,
    Path(id): Path<String>,
) -> Result<Json<SceneDTO>, ApiError> {
    let found = scenes.open(&id).await?;

    Ok(Json(to_dto(&found)))
}

async fn retitle(
    State(scenes): State<SceneService>,
    Path(id): Path<String>,
    Json(request): Json<RetitleSceneRequest>,
) -> Result<Json<SceneDTO>, ApiError> {
    let retitled = scenes.retitle(&id, &request.title).await?;

    Ok(Json(to_dto(&retitled)))
}

async fn link(
    State(scenes): State<SceneService>,
    Path(id): Path<String>,
    Json(request): Json<LinkIdeaRequest>,
) -> Result<Json<SceneDTO>, ApiError> {
    let linked = scenes.link(&id, &request.idea).await?;

    Ok(Json(to_dto(&linked)))
}

async fn unlink(
    State(scenes): State<SceneService>,
    Path((id, idea)): Path<(String, String)>,
) -> Result<Json<SceneDTO>, ApiError> {
    let unlinked = scenes.unlink(&id, &idea).await?;

    Ok(Json(to_dto(&unlinked)))
}

async fn remove(
    State(scenes): State<SceneService>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    scenes.delete(&id).await?;

    Ok(StatusCode::NO_CONTENT)
}

fn to_dto(scene: &Scene) -> SceneDTO {
    SceneDTO {
        id: scene.id().to_string(),
        project: scene.project().to_string(),
        title: scene.title().to_string(),
        ideas: scene.ideas().iter().map(ToString::to_string).collect(),
        text: scene.text(),
    }
}

enum ApiError {
    Scenes(SceneServiceError),
    Catalog(CatalogError),
    Restylings(QuotationRestylingTaskError),
    NeverRestyled(String),
}

impl From<SceneServiceError> for ApiError {
    fn from(error: SceneServiceError) -> Self {
        Self::Scenes(error)
    }
}

impl From<QuotationRestylingTaskError> for ApiError {
    fn from(error: QuotationRestylingTaskError) -> Self {
        Self::Restylings(error)
    }
}

impl From<CatalogError> for ApiError {
    fn from(error: CatalogError) -> Self {
        Self::Catalog(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let error = match self {
            Self::Scenes(error) => error,
            Self::Catalog(error) => {
                tracing::error!(%error, "the scene catalog failed");

                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "the scene catalog is unavailable".to_owned(),
                )
                    .into_response();
            }
            Self::Restylings(error) => {
                tracing::error!(%error, "the quotation restyling tasks failed");

                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "the progress of restyling is unavailable".to_owned(),
                )
                    .into_response();
            }
            Self::NeverRestyled(project) => {
                return (
                    StatusCode::NOT_FOUND,
                    format!("the quotation marks of project {project} were never restyled"),
                )
                    .into_response();
            }
        };
        let (status, message) = match error {
            SceneServiceError::InvalidId(error) => (StatusCode::BAD_REQUEST, error.to_string()),
            SceneServiceError::Untitled(error) => {
                (StatusCode::UNPROCESSABLE_ENTITY, error.to_string())
            }
            SceneServiceError::Store(StoreError::NotFound(id)) => {
                (StatusCode::NOT_FOUND, format!("scene {id} was not found"))
            }
            SceneServiceError::Store(StoreError::Conflict(id)) => {
                (StatusCode::CONFLICT, format!("scene {id} already exists"))
            }
            SceneServiceError::Store(error @ StoreError::Unusable(_)) => {
                (StatusCode::BAD_REQUEST, error.to_string())
            }
            SceneServiceError::Store(error @ StoreError::Backend(_)) => {
                tracing::error!(%error, "the scene store failed");

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "the scene store is unavailable".to_owned(),
                )
            }
        };

        (status, message).into_response()
    }
}
