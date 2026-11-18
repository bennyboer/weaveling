mod telling;
mod transformer;

pub use telling::TellingFollowers;

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::{Query, State};
use axum::routing;
use axum::{Extension, Json, Router};
use eventsourcing::Agent;
use live::{About, AccessWatch, Broadcaster, Gate, GatedTopic, Topic};
use readings_contract::ContentsDTO;
use readings_core::{ProjectLink, ReadingsCore, ReadingsError};
use serde::Deserialize;
use serving::Problem;

struct ReadersGate(ReadingsCore);

#[async_trait]
impl Gate for ReadersGate {
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

/// The changes to one project's live book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookChanges(pub ProjectLink);

impl Display for BookChanges {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

fn book_changes_of(about: &About) -> Option<BookChanges> {
    about
        .get("project")
        .map(|project| BookChanges(ProjectLink::from(project.as_str())))
}

/// Whoever may read a project follows its live book under `readings`, about `{ project }`.
pub fn topic(readings: ReadingsCore, broadcaster: Broadcaster<BookChanges>) -> Arc<dyn Topic> {
    Arc::new(GatedTopic::new(
        "readings",
        broadcaster,
        book_changes_of,
        Arc::new(ReadersGate(readings)),
    ))
}

pub fn router(readings: ReadingsCore) -> Router {
    Router::new()
        .route("/readings/live", routing::get(live_book))
        .with_state(readings)
}

#[derive(Deserialize)]
struct InProject {
    project: String,
}

async fn live_book(
    State(readings): State<ReadingsCore>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<InProject>,
) -> Result<Json<ContentsDTO>, Problem> {
    let book =
        readings
            .live_book(&asked.project, &agent)
            .await
            .map_err(|refusal| match refusal {
                ReadingsError::ProjectNotThere => Problem::not_found(serving::NOT_FOUND, refusal),
                beyond_reach => Problem::unexpected(beyond_reach),
            })?;

    Ok(Json(transformer::to_dto(&book)))
}
