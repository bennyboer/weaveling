use std::sync::Arc;

use async_trait::async_trait;
use authorizing::{Act, Authorizer};
use axum::extract::{Query, State};
use axum::routing;
use axum::{Extension, Json, Router};
use eventsourcing::Agent;
use live::{Broadcaster, ReadersOfTheProject, Teller};
use serde::Deserialize;
use serving::Problem;
use statistics_contract::WordCountDTO;
use statistics_core::{CatalogError, ProjectLink, SceneLink, WordCount, WordCountCatalog};
use time::OffsetDateTime;

#[derive(Clone)]
struct Counts {
    catalog: Arc<dyn WordCountCatalog>,
    authorizer: Arc<dyn Authorizer>,
}

pub fn router(catalog: Arc<dyn WordCountCatalog>, authorizer: Arc<dyn Authorizer>) -> Router {
    Router::new()
        .route("/statistics/word-counts", routing::get(word_counts))
        .with_state(Counts {
            catalog,
            authorizer,
        })
}

pub fn live(broadcaster: Broadcaster, authorizer: Arc<dyn Authorizer>) -> Router {
    live::followed_if(
        "/statistics/word-counts/live",
        broadcaster,
        |asked| asked.get("project").cloned(),
        Arc::new(ReadersOfTheProject(authorizer)),
    )
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

    async fn forget_scene(
        &self,
        scene: &SceneLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.kept.forget_scene(scene, at).await
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

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<WordCount>, CatalogError> {
        self.kept.in_project(project).await
    }
}

#[derive(Deserialize)]
struct InProject {
    project: String,
}

async fn word_counts(
    State(counts): State<Counts>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<InProject>,
) -> Result<Json<Vec<WordCountDTO>>, Problem> {
    if !counts
        .authorizer
        .may(&agent, Act::Read, &asked.project)
        .await
        .map_err(Problem::unexpected)?
    {
        return Err(Problem::not_found(
            serving::NOT_FOUND,
            "there is no such project",
        ));
    }
    let counts = counts
        .catalog
        .in_project(&ProjectLink::from(asked.project))
        .await
        .map_err(Problem::unexpected)?;

    Ok(Json(counts.iter().map(to_dto).collect()))
}

fn to_dto(count: &WordCount) -> WordCountDTO {
    WordCountDTO {
        scene: count.scene.to_string(),
        words: count.words,
    }
}
