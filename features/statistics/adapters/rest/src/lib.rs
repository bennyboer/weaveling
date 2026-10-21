use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use live::{Broadcaster, streaming};
use serde::Deserialize;
use serde_json::json;
use serving::Problem;
use statistics_contract::{ProjectWordCountsDTO, WordCountDTO};
use statistics_core::{CatalogError, ProjectLink, SceneLink, WordCount, WordCountCatalog};
use time::OffsetDateTime;

pub fn router(catalog: Arc<dyn WordCountCatalog>) -> Router {
    Router::new()
        .route("/statistics/word-counts", get(word_counts))
        .with_state(catalog)
}

pub fn live(broadcaster: Broadcaster) -> Router {
    Router::new()
        .route("/statistics/word-counts/live", get(follow))
        .with_state(broadcaster)
}

async fn follow(
    upgrade: WebSocketUpgrade,
    State(broadcaster): State<Broadcaster>,
    Query(followed): Query<InProject>,
) -> Response {
    streaming(upgrade, broadcaster.hear(&followed.project))
}

pub struct TellingWordCounts {
    kept: Arc<dyn WordCountCatalog>,
    broadcaster: Broadcaster,
    told: AtomicU64,
}

impl TellingWordCounts {
    pub fn around(kept: Arc<dyn WordCountCatalog>, broadcaster: Broadcaster) -> Self {
        Self {
            kept,
            broadcaster,
            told: AtomicU64::new(0),
        }
    }

    async fn tell(&self, project: &ProjectLink) {
        let Ok(counts) = self.kept.in_project(project).await else {
            return;
        };
        let version = self.told.fetch_add(1, Ordering::AcqRel) + 1;
        let event = ProjectWordCountsDTO {
            counts: counts.iter().map(to_dto).collect(),
        };
        self.broadcaster.tell(
            &project.to_string(),
            json!({ "version": version, "event": event }).to_string(),
        );
    }
}

#[async_trait]
impl WordCountCatalog for TellingWordCounts {
    async fn keep(
        &self,
        scene: &SceneLink,
        project: &ProjectLink,
        words: u64,
        counted_at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.kept.keep(scene, project, words, counted_at).await?;
        self.tell(project).await;

        Ok(())
    }

    async fn forget_scene(&self, scene: &SceneLink) -> Result<(), CatalogError> {
        self.kept.forget_scene(scene).await
    }

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<WordCount>, CatalogError> {
        self.kept.in_project(project).await
    }
}

#[derive(Deserialize)]
struct InProject {
    project: String,
}

async fn word_counts(
    State(catalog): State<Arc<dyn WordCountCatalog>>,
    Query(asked): Query<InProject>,
) -> Result<Json<Vec<WordCountDTO>>, ApiError> {
    let counts = catalog
        .in_project(&ProjectLink::from(asked.project))
        .await?;

    Ok(Json(counts.iter().map(to_dto).collect()))
}

fn to_dto(count: &WordCount) -> WordCountDTO {
    WordCountDTO {
        scene: count.scene.to_string(),
        words: count.words,
    }
}

struct ApiError(CatalogError);

impl From<CatalogError> for ApiError {
    fn from(error: CatalogError) -> Self {
        Self(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        Problem::unexpected(self.0).into_response()
    }
}
