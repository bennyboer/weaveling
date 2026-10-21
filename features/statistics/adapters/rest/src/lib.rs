use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use live::{Broadcaster, Teller, followed_at};
use serde::Deserialize;
use serving::Problem;
use statistics_contract::WordCountDTO;
use statistics_core::{CatalogError, ProjectLink, SceneLink, WordCount, WordCountCatalog};
use time::OffsetDateTime;

pub fn router(catalog: Arc<dyn WordCountCatalog>) -> Router {
    Router::new()
        .route("/statistics/word-counts", get(word_counts))
        .with_state(catalog)
}

pub fn live(broadcaster: Broadcaster) -> Router {
    followed_at("/statistics/word-counts/live", broadcaster, |asked| {
        asked.get("project").cloned()
    })
}

pub struct TellingWordCounts {
    kept: Arc<dyn WordCountCatalog>,
    teller: Teller,
}

impl TellingWordCounts {
    pub fn around(kept: Arc<dyn WordCountCatalog>, broadcaster: Broadcaster) -> Self {
        Self {
            kept,
            teller: Teller::new(broadcaster),
        }
    }

    fn tell(&self, project: &ProjectLink, scene: &SceneLink, words: u64) {
        let event = WordCountDTO {
            scene: scene.to_string(),
            words,
        };
        self.teller
            .tell(&project.to_string(), &scene.to_string(), event);
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
    ) -> Result<Option<u64>, CatalogError> {
        let kept = self.kept.keep(scene, project, words, counted_at).await?;
        if let Some(words) = kept {
            self.tell(project, scene, words);
        }

        Ok(kept)
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
