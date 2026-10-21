use std::sync::Arc;

use appearances_contract::{IdeaPlacesDTO, PlaceDTO};
use appearances_core::{AppearanceCatalog, CatalogError, IdeaLink, Place, Subject};
use async_trait::async_trait;
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use live::{Broadcaster, Teller, followed_at};
use serde::Deserialize;
use serving::Problem;

pub fn router(catalog: Arc<dyn AppearanceCatalog>) -> Router {
    Router::new()
        .route("/appearances", get(places_of))
        .with_state(catalog)
}

pub fn live(broadcaster: Broadcaster) -> Router {
    followed_at("/appearances/live", broadcaster, |asked| {
        asked.get("idea").cloned()
    })
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
) -> Result<Json<Vec<PlaceDTO>>, Problem> {
    let places = catalog
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
