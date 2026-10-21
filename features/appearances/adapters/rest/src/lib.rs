use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use appearances_contract::{IdeaPlacesDTO, PlaceDTO};
use appearances_core::{AppearanceCatalog, CatalogError, IdeaLink, Place, Subject};
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

pub fn router(catalog: Arc<dyn AppearanceCatalog>) -> Router {
    Router::new()
        .route("/appearances", get(places_of))
        .with_state(catalog)
}

pub fn live(broadcaster: Broadcaster) -> Router {
    Router::new()
        .route("/appearances/live", get(follow))
        .with_state(broadcaster)
}

async fn follow(
    upgrade: WebSocketUpgrade,
    State(broadcaster): State<Broadcaster>,
    Query(followed): Query<OfIdea>,
) -> Response {
    streaming(upgrade, broadcaster.hear(&followed.idea))
}

pub struct TellingAppearances {
    kept: Arc<dyn AppearanceCatalog>,
    broadcaster: Broadcaster,
    told: AtomicU64,
}

impl TellingAppearances {
    pub fn around(kept: Arc<dyn AppearanceCatalog>, broadcaster: Broadcaster) -> Self {
        Self {
            kept,
            broadcaster,
            told: AtomicU64::new(0),
        }
    }

    async fn tell(&self, subject: &Subject) {
        let Ok(places) = self.kept.places_of(subject).await else {
            return;
        };
        let Subject::Idea(idea) = subject;
        let version = self.told.fetch_add(1, Ordering::AcqRel) + 1;
        let event = IdeaPlacesDTO {
            places: places.iter().map(to_dto).collect(),
        };
        self.broadcaster.tell(
            idea.as_str(),
            json!({ "of": idea.as_str(), "version": version, "event": event }).to_string(),
        );
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

    async fn forget_subject(&self, subject: &Subject) -> Result<(), CatalogError> {
        self.kept.forget_subject(subject).await?;
        self.tell(subject).await;

        Ok(())
    }

    async fn forget_place(&self, place: &Place) -> Result<Vec<Subject>, CatalogError> {
        let appeared = self.kept.forget_place(place).await?;
        for subject in &appeared {
            self.tell(subject).await;
        }

        Ok(appeared)
    }

    async fn places_of(&self, subject: &Subject) -> Result<Vec<Place>, CatalogError> {
        self.kept.places_of(subject).await
    }
}

#[derive(Deserialize)]
struct OfIdea {
    idea: String,
}

async fn places_of(
    State(catalog): State<Arc<dyn AppearanceCatalog>>,
    Query(asked): Query<OfIdea>,
) -> Result<Json<Vec<PlaceDTO>>, ApiError> {
    let places = catalog
        .places_of(&Subject::Idea(IdeaLink::from(asked.idea)))
        .await?;

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
