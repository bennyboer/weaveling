use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

use async_trait::async_trait;

use axum::Extension;
use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::header::ETAG;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing;
use eventsourcing::{Agent, Standing, Version};
use live::{About, AccessWatch, Broadcaster, Gate, GatedTopic, Teller, Topic};
use scenes_contract::{
    CreateSceneRequest, LinkIdeaRequest, QuotationRestylingDTO, RetitleSceneRequest, SceneDTO,
    ScenePageDTO, SceneSummaryDTO, refused,
};
use scenes_core::{
    CatalogError, InvalidSceneTitle, Page, ProjectLink, QuotationRestylingProgress,
    QuotationRestylingStanding, QuotationRestylingTaskError, QuotationRestylingTasks, Scene,
    SceneCatalog, SceneId, SceneSummary, SceneTitle, ScenesCore, ScenesError, StoreError,
};
use serde::Deserialize;
use serving::{NOT_FOUND, Problem};
use time::OffsetDateTime;
use transacting::Transaction;

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

    async fn watch(&self, project: &str) -> AccessWatch {
        self.0.watch_access(project)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneChanges(pub ProjectLink);

impl Display for SceneChanges {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestylingChanges(pub ProjectLink);

impl Display for RestylingChanges {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

fn scene_changes_of(about: &About) -> Option<SceneChanges> {
    about
        .get("project")
        .map(|project| SceneChanges(ProjectLink::from(project.as_str())))
}

fn restyling_changes_of(about: &About) -> Option<RestylingChanges> {
    about
        .get("project")
        .map(|project| RestylingChanges(ProjectLink::from(project.as_str())))
}

pub fn scenes_topic(scenes: ScenesCore, broadcaster: Broadcaster<SceneChanges>) -> Arc<dyn Topic> {
    Arc::new(GatedTopic::new(
        "scenes",
        broadcaster,
        scene_changes_of,
        Arc::new(ScenesGate(scenes)),
    ))
}

pub fn restylings_topic(
    scenes: ScenesCore,
    broadcaster: Broadcaster<RestylingChanges>,
) -> Arc<dyn Topic> {
    Arc::new(GatedTopic::new(
        "quotation-restylings",
        broadcaster,
        restyling_changes_of,
        Arc::new(ScenesGate(scenes)),
    ))
}

pub struct TellingScenes {
    kept: Arc<dyn SceneCatalog>,
    teller: Arc<Teller<SceneChanges>>,
}

impl TellingScenes {
    pub fn around(kept: Arc<dyn SceneCatalog>, broadcaster: Broadcaster<SceneChanges>) -> Self {
        Self {
            kept,
            teller: Arc::new(Teller::new(broadcaster)),
        }
    }

    fn tell_after(&self, transaction: &mut Transaction, id: SceneId, project: &ProjectLink) {
        let (kept, teller, project) = (self.kept.clone(), self.teller.clone(), project.clone());

        transaction.after_commit(async move {
            teller
                .tell_latest(SceneChanges(project), &id.to_string(), async {
                    kept.summary_of(id)
                        .await
                        .ok()
                        .flatten()
                        .map(|summary| to_summary_dto(&summary))
                })
                .await;
        });
    }
}

#[async_trait]
impl SceneCatalog for TellingScenes {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        project: &ProjectLink,
    ) -> Result<(), CatalogError> {
        self.kept.remember(transaction, id, project).await?;
        self.tell_after(transaction, id, project);

        Ok(())
    }

    async fn retitle(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        project: &ProjectLink,
        title: &SceneTitle,
        version: u64,
    ) -> Result<(), CatalogError> {
        self.kept
            .retitle(transaction, id, project, title, version)
            .await?;
        self.tell_after(transaction, id, project);

        Ok(())
    }

    async fn note_opening(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        project: &ProjectLink,
        opening: &str,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.kept
            .note_opening(transaction, id, project, opening, at)
            .await?;
        self.tell_after(transaction, id, project);

        Ok(())
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.kept.forget(transaction, id, at).await
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.kept
            .forget_tombstones_laid_before(transaction, before, at_most)
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

    async fn lay_project_tombstone(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.kept
            .lay_project_tombstone(transaction, project, at)
            .await
    }

    async fn has_project_tombstone(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
        self.kept.has_project_tombstone(project).await
    }

    async fn has_project_tombstone_within(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
    ) -> Result<bool, CatalogError> {
        self.kept
            .has_project_tombstone_within(transaction, project)
            .await
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

pub fn quotation_restylings(scenes: ScenesCore) -> Router {
    Router::new()
        .route(
            "/scenes/quotation-restylings/{project}",
            routing::get(progress),
        )
        .with_state(scenes)
}

pub struct TellingRestylingTasks {
    kept: Arc<dyn QuotationRestylingTasks>,
    teller: Arc<Teller<RestylingChanges>>,
}

impl TellingRestylingTasks {
    pub fn around(
        kept: Arc<dyn QuotationRestylingTasks>,
        broadcaster: Broadcaster<RestylingChanges>,
    ) -> Self {
        Self {
            kept,
            teller: Arc::new(Teller::new(broadcaster)),
        }
    }

    fn tell_after(&self, transaction: &mut Transaction, project: &ProjectLink) {
        let (kept, teller, project) = (self.kept.clone(), self.teller.clone(), project.clone());

        transaction.after_commit(async move {
            teller
                .tell_latest(RestylingChanges(project.clone()), project.as_str(), async {
                    kept.progress(&project).await.ok().flatten().map(as_dto)
                })
                .await;
        });
    }
}

#[async_trait]
impl QuotationRestylingTasks for TellingRestylingTasks {
    async fn begin(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        change: u64,
        total: u64,
        cursor: Option<&str>,
    ) -> Result<bool, QuotationRestylingTaskError> {
        let begun = self
            .kept
            .begin(transaction, project, change, total, cursor)
            .await?;
        if begun {
            self.tell_after(transaction, project);
        }

        Ok(begun)
    }

    async fn standing(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
    ) -> Result<Option<QuotationRestylingStanding>, QuotationRestylingTaskError> {
        self.kept.standing(transaction, project).await
    }

    async fn progressed(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        change: u64,
        page: Page<'_>,
        finished: bool,
    ) -> Result<bool, QuotationRestylingTaskError> {
        let counted = self
            .kept
            .progressed(transaction, project, change, page, finished)
            .await?;
        if counted {
            self.tell_after(transaction, project);
        }

        Ok(counted)
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
) -> Result<Response, Problem> {
    let created = scenes
        .create(&request.project, &acting)
        .await
        .map_err(problem_of)?;

    Ok(to_response(
        StatusCode::CREATED,
        &Standing {
            state: created,
            version: Version::ZERO,
        },
    ))
}

async fn find(
    State(scenes): State<ScenesCore>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
) -> Result<Response, Problem> {
    let found = scenes.read(&id, &acting).await.map_err(problem_of)?;

    Ok(to_response(StatusCode::OK, &found))
}

async fn retitle(
    State(scenes): State<ScenesCore>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RetitleSceneRequest>,
) -> Result<Response, Problem> {
    let retitled = scenes
        .retitle(&id, &request.title, serving::demanded(&headers)?, &acting)
        .await
        .map_err(problem_of)?;

    Ok(to_response(StatusCode::OK, &retitled))
}

async fn link(
    State(scenes): State<ScenesCore>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<LinkIdeaRequest>,
) -> Result<Response, Problem> {
    let linked = scenes
        .link(&id, &request.idea, serving::demanded(&headers)?, &acting)
        .await
        .map_err(problem_of)?;

    Ok(to_response(StatusCode::OK, &linked))
}

async fn unlink(
    State(scenes): State<ScenesCore>,
    Extension(acting): Extension<Agent>,
    Path((id, idea)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let unlinked = scenes
        .unlink(&id, &idea, serving::demanded(&headers)?, &acting)
        .await
        .map_err(problem_of)?;

    Ok(to_response(StatusCode::OK, &unlinked))
}

async fn remove(
    State(scenes): State<ScenesCore>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    scenes
        .delete(&id, serving::demanded(&headers)?, &acting)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

fn to_response(status: StatusCode, standing: &Standing<Scene>) -> Response {
    let scene = &standing.state;

    (
        status,
        [(ETAG, serving::tag(standing.version))],
        Json(SceneDTO {
            id: scene.id().to_string(),
            version: standing.version.count(),
            project: scene.project().to_string(),
            title: scene.title().to_string(),
            ideas: scene.ideas().iter().map(ToString::to_string).collect(),
            text: scene.text(),
        }),
    )
        .into_response()
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
        ScenesError::IdeaNotThere => Problem::not_found(NOT_FOUND, ScenesError::IdeaNotThere),
        ScenesError::NotAnAuthor => {
            Problem::forbidden(refused::NOT_AN_AUTHOR, ScenesError::NotAnAuthor)
        }
        ScenesError::Outdated { .. } => Problem::outdated("scene"),
        beyond_reach => Problem::unexpected(beyond_reach),
    }
}
