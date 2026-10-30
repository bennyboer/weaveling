use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::{Query, State};
use axum::routing;
use axum::{Extension, Json, Router};
use eventsourcing::Agent;
use live::{AccessWatch, Broadcaster, Gate, Teller};
use serde::Deserialize;
use serving::Problem;
use statistics_contract::WordCountDTO;
use statistics_core::{
    CatalogError, ProjectLink, SceneLink, StatisticsCore, StatisticsError, WordCount,
    WordCountCatalog,
};
use time::OffsetDateTime;

pub fn router(statistics: StatisticsCore) -> Router {
    Router::new()
        .route("/statistics/word-counts", routing::get(word_counts))
        .with_state(statistics)
}

struct StatisticsGate(StatisticsCore);

#[async_trait]
impl Gate for StatisticsGate {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordCountChanges(pub ProjectLink);

impl Display for WordCountChanges {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

pub fn live(statistics: StatisticsCore, broadcaster: Broadcaster<WordCountChanges>) -> Router {
    live::followed_while(
        "/statistics/word-counts/live",
        broadcaster,
        |asked| {
            asked
                .get("project")
                .map(|project| WordCountChanges(ProjectLink::from(project.as_str())))
        },
        Arc::new(StatisticsGate(statistics)),
    )
}

pub struct TellingWordCounts {
    kept: Arc<dyn WordCountCatalog>,
    teller: Teller<WordCountChanges>,
}

impl TellingWordCounts {
    pub fn around(
        kept: Arc<dyn WordCountCatalog>,
        broadcaster: Broadcaster<WordCountChanges>,
    ) -> Self {
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
            .tell(WordCountChanges(project.clone()), &scene.to_string(), event);
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
    State(statistics): State<StatisticsCore>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<InProject>,
) -> Result<Json<Vec<WordCountDTO>>, Problem> {
    let counts = statistics
        .word_counts(&asked.project, &agent)
        .await
        .map_err(|refusal| match refusal {
            StatisticsError::ProjectNotThere => Problem::not_found(serving::NOT_FOUND, refusal),
            beyond_reach => Problem::unexpected(beyond_reach),
        })?;

    Ok(Json(counts.iter().map(to_dto).collect()))
}

fn to_dto(count: &WordCount) -> WordCountDTO {
    WordCountDTO {
        scene: count.scene.to_string(),
        words: count.words,
    }
}
