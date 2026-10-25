use std::sync::Arc;

use appearances_contract::{IdeaPlacesDTO, PlaceDTO};
use appearances_core::{AppearanceCatalog, CatalogError, IdeaLink, Place, ProjectLink, Subject};
use async_trait::async_trait;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing;
use axum::{Extension, Json, Router};
use eventsourcing::Agent;
use live::{Asked, Broadcaster, Teller};

use crate::guarding::ReadableBy;
use serde::Deserialize;
use serving::Problem;
use time::OffsetDateTime;

mod guarding;

pub use guarding::Readers;

pub fn router(readers: Readers) -> Router {
    Router::new()
        .route("/appearances", routing::get(places_of))
        .with_state(readers)
}

#[derive(Clone)]
struct Following {
    broadcaster: Broadcaster,
    readers: Readers,
}

pub fn live(broadcaster: Broadcaster, readers: Readers) -> Router {
    Router::new()
        .route("/appearances/live", routing::get(follow))
        .with_state(Following {
            broadcaster,
            readers,
        })
}

async fn follow(
    upgrade: WebSocketUpgrade,
    State(following): State<Following>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<Asked>,
) -> Response {
    let Some(idea) = asked.get("idea") else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    match following.readers.may_read(&agent, idea).await {
        Ok(Some(false)) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        Ok(_) => live::streaming_sifted(
            upgrade,
            following.broadcaster.hear(idea),
            Arc::new(ReadableBy {
                agent,
                readers: following.readers,
            }),
        ),
    }
}

pub struct TellingAppearances {
    kept: Arc<dyn AppearanceCatalog>,
    teller: Teller,
}

impl TellingAppearances {
    pub fn around(kept: Arc<dyn AppearanceCatalog>, broadcaster: Broadcaster) -> Self {
        Self {
            kept,
            teller: Teller::new(broadcaster),
        }
    }

    async fn tell(&self, subject: &Subject) {
        let Ok(places) = self.kept.places_of(subject).await else {
            return;
        };
        let Subject::Idea(idea) = subject;
        let event = IdeaPlacesDTO {
            places: places.iter().map(to_dto).collect(),
        };
        self.teller.tell(idea.as_str(), idea.as_str(), event);
    }
}

#[async_trait]
impl AppearanceCatalog for TellingAppearances {
    async fn remember(
        &self,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError> {
        self.kept.remember(subject, place, version).await?;
        self.tell(subject).await;

        Ok(())
    }

    async fn forget(
        &self,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError> {
        self.kept.forget(subject, place, version).await?;
        self.tell(subject).await;

        Ok(())
    }

    async fn forget_subject(
        &self,
        subject: &Subject,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        self.kept.forget_subject(subject, at).await?;
        self.tell(subject).await;

        Ok(())
    }

    async fn forget_place(
        &self,
        place: &Place,
        at: OffsetDateTime,
    ) -> Result<Vec<Subject>, CatalogError> {
        let appeared = self.kept.forget_place(place, at).await?;
        for subject in &appeared {
            self.tell(subject).await;
        }

        Ok(appeared)
    }

    async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
    ) -> Result<u64, CatalogError> {
        self.kept.forget_tombstones_laid_before(before).await
    }

    async fn places_of(&self, subject: &Subject) -> Result<Vec<Place>, CatalogError> {
        self.kept.places_of(subject).await
    }

    async fn remember_project(
        &self,
        subject: &Subject,
        project: &ProjectLink,
    ) -> Result<(), CatalogError> {
        self.kept.remember_project(subject, project).await
    }

    async fn project_of(&self, subject: &Subject) -> Result<Option<ProjectLink>, CatalogError> {
        self.kept.project_of(subject).await
    }
}

#[derive(Deserialize)]
struct OfIdea {
    idea: String,
}

async fn places_of(
    State(readers): State<Readers>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<OfIdea>,
) -> Result<Json<Vec<PlaceDTO>>, Problem> {
    if readers
        .may_read(&agent, &asked.idea)
        .await
        .map_err(Problem::unexpected)?
        == Some(false)
    {
        return Err(Problem::not_found(
            serving::NOT_FOUND,
            "there is no such idea",
        ));
    }
    let places = readers
        .catalog
        .places_of(&Subject::Idea(IdeaLink::from(asked.idea)))
        .await
        .map_err(Problem::unexpected)?;

    Ok(Json(places.iter().map(to_dto).collect()))
}

fn to_dto(place: &Place) -> PlaceDTO {
    match place {
        Place::Scene(scene) => PlaceDTO::Scene {
            id: scene.to_string(),
        },
        Place::Section(section) => PlaceDTO::Section {
            id: section.to_string(),
        },
    }
}
