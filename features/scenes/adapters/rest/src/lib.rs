use std::sync::Arc;

use async_trait::async_trait;

use axum::Json;
use axum::Router;
use axum::extract::{FromRef, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use live::{Broadcaster, Teller, followed_at};
use scenes_contract::{
    CreateSceneRequest, LinkIdeaRequest, QuotationRestylingDTO, RetitleSceneRequest, SceneDTO,
    ScenePageDTO, SceneSummaryDTO, refused,
};
use scenes_core::{
    CatalogError, InvalidSceneTitle, ProjectLink, QuotationRestylingProgress,
    QuotationRestylingTaskError, QuotationRestylingTasks, Scene, SceneCatalog, SceneId,
    SceneService, SceneServiceError, SceneSummary, SceneTitle, StoreError,
};
use serde::Deserialize;
use serving::{NOT_FOUND, Problem};
use time::OffsetDateTime;

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

pub fn live_scenes(broadcaster: Broadcaster) -> Router {
    followed_at("/scenes/live", broadcaster, |asked| {
        asked.get("project").cloned()
    })
}

pub struct TellingScenes {
    kept: Arc<dyn SceneCatalog>,
    teller: Teller,
}

impl TellingScenes {
    pub fn around(kept: Arc<dyn SceneCatalog>, broadcaster: Broadcaster) -> Self {
        Self {
            kept,
            teller: Teller::new(broadcaster),
        }
    }

    async fn tell(&self, id: SceneId) {
        let Ok(Some(summary)) = self.kept.summary_of(id).await else {
            return;
        };
        self.teller.tell(
            &summary.project.to_string(),
            &id.to_string(),
            to_summary_dto(&summary),
        );
    }
}

#[async_trait]
impl SceneCatalog for TellingScenes {
    async fn remember(&self, id: SceneId, project: &ProjectLink) -> Result<(), CatalogError> {
        self.kept.remember(id, project).await?;
        self.tell(id).await;

        Ok(())
    }

    async fn retitle(
        &self,
        id: SceneId,
        project: &ProjectLink,
        title: &SceneTitle,
        version: u64,
    ) -> Result<(), CatalogError> {
        self.kept.retitle(id, project, title, version).await?;
        self.tell(id).await;

        Ok(())
    }

    async fn note_opening(
        &self,
        id: SceneId,
        project: &ProjectLink,
        opening: &str,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.kept.note_opening(id, project, opening, at).await?;
        self.tell(id).await;

        Ok(())
    }

    async fn forget(&self, id: SceneId) -> Result<(), CatalogError> {
        self.kept.forget(id).await
    }

    async fn summary_of(&self, id: SceneId) -> Result<Option<SceneSummary>, CatalogError> {
        self.kept.summary_of(id).await
    }

    async fn in_project_after(
        &self,
        project: &ProjectLink,
        after: Option<SceneId>,
        at_most: usize,
    ) -> Result<Vec<SceneSummary>, CatalogError> {
        self.kept.in_project_after(project, after, at_most).await
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

pub fn quotation_restylings(
    tasks: Arc<dyn QuotationRestylingTasks>,
    broadcaster: Broadcaster,
) -> Router {
    Router::new()
        .route("/scenes/quotation-restylings/{project}", get(progress))
        .with_state(tasks)
        .merge(followed_at(
            "/scenes/quotation-restylings/live",
            broadcaster,
            |asked| asked.get("project").cloned(),
        ))
}

pub struct TellingRestylingTasks {
    kept: Arc<dyn QuotationRestylingTasks>,
    teller: Teller,
}

impl TellingRestylingTasks {
    pub fn around(kept: Arc<dyn QuotationRestylingTasks>, broadcaster: Broadcaster) -> Self {
        Self {
            kept,
            teller: Teller::new(broadcaster),
        }
    }

    async fn tell(&self, project: &ProjectLink) {
        let Ok(Some(progress)) = self.kept.progress(project).await else {
            return;
        };
        self.teller
            .tell(project.as_str(), project.as_str(), as_dto(progress));
    }
}

#[async_trait]
impl QuotationRestylingTasks for TellingRestylingTasks {
    async fn begin(
        &self,
        project: &ProjectLink,
        change: u64,
        total: u64,
    ) -> Result<bool, QuotationRestylingTaskError> {
        let begun = self.kept.begin(project, change, total).await?;
        if begun {
            self.tell(project).await;
        }

        Ok(begun)
    }

    async fn claim(
        &self,
        project: &ProjectLink,
        change: u64,
    ) -> Result<bool, QuotationRestylingTaskError> {
        self.kept.claim(project, change).await
    }

    async fn progressed(
        &self,
        project: &ProjectLink,
        change: u64,
        done: u64,
        finished: bool,
    ) -> Result<(), QuotationRestylingTaskError> {
        self.kept
            .progressed(project, change, done, finished)
            .await?;
        self.tell(project).await;

        Ok(())
    }

    async fn progress(
        &self,
        project: &ProjectLink,
    ) -> Result<Option<QuotationRestylingProgress>, QuotationRestylingTaskError> {
        self.kept.progress(project).await
    }
}

fn as_dto(progress: QuotationRestylingProgress) -> QuotationRestylingDTO {
    QuotationRestylingDTO {
        change: progress.change,
        total: progress.total,
        done: progress.done.min(progress.total),
        finished: progress.finished,
    }
}

async fn progress(
    State(tasks): State<Arc<dyn QuotationRestylingTasks>>,
    Path(project): Path<String>,
) -> Result<Json<QuotationRestylingDTO>, ApiError> {
    let progress = tasks
        .progress(&ProjectLink::from(project.as_str()))
        .await?
        .ok_or(ApiError::NeverRestyled(project))?;

    Ok(Json(as_dto(progress)))
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
        match self {
            Self::Scenes(refusal) => problem_of(refusal),
            Self::Catalog(failure) => Problem::unexpected(failure),
            Self::Restylings(failure) => Problem::unexpected(failure),
            Self::NeverRestyled(project) => Problem::not_found(
                refused::NEVER_RESTYLED,
                format!("the quotation marks of project {project} were never restyled"),
            ),
        }
        .into_response()
    }
}

fn problem_of(refusal: SceneServiceError) -> Problem {
    match refusal {
        SceneServiceError::InvalidId(reason) => Problem::rejected(refused::ID_INVALID, reason),
        SceneServiceError::Untitled(reason) => match reason {
            InvalidSceneTitle::ControlCharacter => {
                Problem::unprocessable(refused::TITLE_CONTROL_CHARACTER, reason)
            }
            InvalidSceneTitle::TooLong { max, actual } => {
                Problem::unprocessable(refused::TITLE_TOO_LONG, reason)
                    .with("max", max)
                    .with("actual", actual)
            }
        },
        SceneServiceError::Store(missing @ StoreError::NotFound(_)) => {
            Problem::not_found(NOT_FOUND, missing)
        }
        beyond_reach => Problem::unexpected(beyond_reach),
    }
}
