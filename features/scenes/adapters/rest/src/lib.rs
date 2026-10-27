use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;

use axum::Extension;
use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing;
use eventsourcing::Agent;
use live::{AccessWatch, Broadcaster, Gate, Teller};
use scenes_contract::{
    CreateSceneRequest, LinkIdeaRequest, QuotationRestylingDTO, RetitleSceneRequest, SceneDTO,
    ScenePageDTO, SceneSummaryDTO, refused,
};
use scenes_core::{
    CatalogError, InvalidSceneTitle, ProjectLink, QuotationRestylingProgress,
    QuotationRestylingTaskError, QuotationRestylingTasks, Scene, SceneCatalog, SceneId,
    SceneSummary, SceneTitle, ScenesCore, ScenesError, StoreError,
};
use serde::Deserialize;
use serving::{NOT_FOUND, Problem};
use time::OffsetDateTime;

struct ScenesGate(ScenesCore);

#[async_trait]
impl Gate for ScenesGate {
    async fn admits(
        &self,
        agent: &Agent,
        project: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        Ok(self.0.may_follow(project, agent).await?)
    }

    fn watch(&self, project: &str) -> AccessWatch {
        self.0.watch_access(project)
    }
}

pub fn live_scenes(scenes: ScenesCore, broadcaster: Broadcaster) -> Router {
    live::followed_while(
        "/scenes/live",
        broadcaster,
        |asked| asked.get("project").cloned(),
        Arc::new(ScenesGate(scenes)),
    )
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

    async fn forget(&self, id: SceneId, at: OffsetDateTime) -> Result<(), CatalogError> {
        self.kept.forget(id, at).await
    }

    async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.kept
            .forget_tombstones_laid_before(before, at_most)
            .await
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

pub fn router(scenes: ScenesCore) -> Router {
    Router::new()
        .route("/scenes", routing::get(list).post(create))
        .route(
            "/scenes/{id}",
            routing::get(find).patch(retitle).delete(remove),
        )
        .route("/scenes/{id}/ideas", routing::post(link))
        .route("/scenes/{id}/ideas/{idea}", routing::delete(unlink))
        .with_state(scenes)
}

pub fn quotation_restylings(scenes: ScenesCore, broadcaster: Broadcaster) -> Router {
    Router::new()
        .route(
            "/scenes/quotation-restylings/{project}",
            routing::get(progress),
        )
        .with_state(scenes.clone())
        .merge(live::followed_while(
            "/scenes/quotation-restylings/live",
            broadcaster,
            |asked| asked.get("project").cloned(),
            Arc::new(ScenesGate(scenes)),
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
    State(scenes): State<ScenesCore>,
    Extension(agent): Extension<Agent>,
    Path(project): Path<String>,
) -> Result<Json<QuotationRestylingDTO>, Problem> {
    let progress = scenes
        .restyling_progress(&project, &agent)
        .await
        .map_err(problem_of)?
        .ok_or_else(|| {
            Problem::not_found(
                refused::NEVER_RESTYLED,
                format!("the quotation marks of project {project} were never restyled"),
            )
        })?;

    Ok(Json(as_dto(progress)))
}

pub const PAGE: usize = 1000;

#[derive(Deserialize)]
struct InProject {
    project: String,
    after: Option<String>,
}

async fn list(
    State(scenes): State<ScenesCore>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<InProject>,
) -> Result<Json<ScenePageDTO>, Problem> {
    let mut found = scenes
        .list(&asked.project, asked.after.as_deref(), PAGE + 1, &agent)
        .await
        .map_err(problem_of)?;
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
    State(scenes): State<ScenesCore>,
    Extension(acting): Extension<Agent>,
    Json(request): Json<CreateSceneRequest>,
) -> Result<(StatusCode, Json<SceneDTO>), Problem> {
    let created = scenes
        .create(&request.project, &acting)
        .await
        .map_err(problem_of)?;

    Ok((StatusCode::CREATED, Json(to_dto(&created))))
}

async fn find(
    State(scenes): State<ScenesCore>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
) -> Result<Json<SceneDTO>, Problem> {
    let found = scenes.read(&id, &acting).await.map_err(problem_of)?;

    Ok(Json(to_dto(&found)))
}

async fn retitle(
    State(scenes): State<ScenesCore>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
    Json(request): Json<RetitleSceneRequest>,
) -> Result<Json<SceneDTO>, Problem> {
    let retitled = scenes
        .retitle(&id, &request.title, &acting)
        .await
        .map_err(problem_of)?;

    Ok(Json(to_dto(&retitled)))
}

async fn link(
    State(scenes): State<ScenesCore>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
    Json(request): Json<LinkIdeaRequest>,
) -> Result<Json<SceneDTO>, Problem> {
    let linked = scenes
        .link(&id, &request.idea, &acting)
        .await
        .map_err(problem_of)?;

    Ok(Json(to_dto(&linked)))
}

async fn unlink(
    State(scenes): State<ScenesCore>,
    Extension(acting): Extension<Agent>,
    Path((id, idea)): Path<(String, String)>,
) -> Result<Json<SceneDTO>, Problem> {
    let unlinked = scenes
        .unlink(&id, &idea, &acting)
        .await
        .map_err(problem_of)?;

    Ok(Json(to_dto(&unlinked)))
}

async fn remove(
    State(scenes): State<ScenesCore>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
) -> Result<StatusCode, Problem> {
    scenes.delete(&id, &acting).await.map_err(problem_of)?;

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

fn problem_of(refusal: ScenesError) -> Problem {
    match refusal {
        ScenesError::InvalidId(reason) => Problem::rejected(refused::ID_INVALID, reason),
        ScenesError::Untitled(reason) => match reason {
            InvalidSceneTitle::Blank => Problem::unexpected(reason),
            InvalidSceneTitle::ControlCharacter => {
                Problem::unprocessable(refused::TITLE_CONTROL_CHARACTER, reason)
            }
            InvalidSceneTitle::TooLong { max, actual } => {
                Problem::unprocessable(refused::TITLE_TOO_LONG, reason)
                    .with("max", max)
                    .with("actual", actual)
            }
        },
        ScenesError::Store(missing @ StoreError::NotFound(_)) => {
            Problem::not_found(NOT_FOUND, missing)
        }
        ScenesError::ProjectNotThere => Problem::not_found(NOT_FOUND, ScenesError::ProjectNotThere),
        ScenesError::NotAnAuthor => {
            Problem::forbidden(refused::NOT_AN_AUTHOR, ScenesError::NotAnAuthor)
        }
        beyond_reach => Problem::unexpected(beyond_reach),
    }
}
