mod telling;
mod transformer;

pub use telling::TellingFollowers;

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::Path;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing;
use axum::{Extension, Json, Router};
use eventsourcing::Agent;
use live::{About, AccessWatch, Broadcaster, Gate, GatedTopic, Topic};
use readings_contract::{
    ContentsDTO, ListedReadingCopyDTO, MakeReadingCopyRequest, ReadingCopyDTO, SharedDTO, refused,
};
use readings_core::{ProjectLink, ReadingsCore, ReadingsError, SceneLink, SectionLink, Shared};
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
        .route("/readings/copies", routing::get(copies).post(make_copy))
        .route("/readings/copies/{copy}", routing::get(copy))
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
    let book = readings
        .live_book(&asked.project, &agent)
        .await
        .map_err(problem_of)?;

    Ok(Json(transformer::to_dto(&book)))
}

async fn copies(
    State(readings): State<ReadingsCore>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<InProject>,
) -> Result<Json<Vec<ListedReadingCopyDTO>>, Problem> {
    let listed = readings
        .copies_of(&asked.project, &agent)
        .await
        .map_err(problem_of)?;

    Ok(Json(
        listed
            .iter()
            .map(|copy| transformer::to_listed_reading_copy_dto(copy, &agent))
            .collect(),
    ))
}

async fn copy(
    State(readings): State<ReadingsCore>,
    Extension(agent): Extension<Agent>,
    Path(copy): Path<String>,
) -> Result<Json<ReadingCopyDTO>, Problem> {
    let (listed, contents) = readings.copy(&copy, &agent).await.map_err(problem_of)?;

    Ok(Json(transformer::to_reading_copy_dto(
        &listed, &contents, &agent,
    )))
}

async fn make_copy(
    State(readings): State<ReadingsCore>,
    Extension(agent): Extension<Agent>,
    Json(asked): Json<MakeReadingCopyRequest>,
) -> Result<(StatusCode, Json<ListedReadingCopyDTO>), Problem> {
    let shared = match asked.shared {
        SharedDTO::Book => Shared::Book,
        SharedDTO::Sections { first, last } => Shared::Sections {
            first: SectionLink::from(first),
            last: SectionLink::from(last),
        },
        SharedDTO::Scenes { first, last } => Shared::Scenes {
            first: SceneLink::from(first),
            last: SceneLink::from(last),
        },
    };
    let made = readings
        .make_copy(
            &asked.project,
            &asked.name,
            &shared,
            moment_in(asked.open_until.as_deref())?,
            moment_in(asked.deleted_on.as_deref())?,
            &agent,
        )
        .await
        .map_err(problem_of)?;

    Ok((
        StatusCode::CREATED,
        Json(transformer::to_listed_reading_copy_dto(&made, &agent)),
    ))
}

fn moment_in(written: Option<&str>) -> Result<Option<time::OffsetDateTime>, Problem> {
    written
        .map(|written| {
            clock::text::read(written).ok_or_else(|| {
                Problem::unprocessable(
                    refused::MOMENT_UNREADABLE,
                    format!("{written} is not a moment as RFC 3339 writes it"),
                )
            })
        })
        .transpose()
}

fn problem_of(refusal: ReadingsError) -> Problem {
    match refusal {
        ReadingsError::ProjectNotThere | ReadingsError::ReadingCopyNotThere => {
            Problem::not_found(serving::NOT_FOUND, refusal)
        }
        beyond_reach => Problem::unexpected(beyond_reach),
    }
}
