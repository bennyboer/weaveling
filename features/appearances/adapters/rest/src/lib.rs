use std::sync::Arc;

use appearances_contract::{IdeaPlacesDTO, PlaceDTO};
use appearances_core::{
    AppearanceCatalog, AppearancesCore, AppearancesError, CatalogError, Place, ProjectLink, Subject,
};
use async_trait::async_trait;
use axum::extract::{Query, State};
use axum::routing;
use axum::{Extension, Json, Router};
use eventsourcing::Agent;
use live::{About, Broadcaster, Refused, Subscribed, Teller, Topic};

use crate::guarding::ReadableBy;
use serde::Deserialize;
use serving::Problem;
use time::OffsetDateTime;

mod guarding;

pub fn router(appearances: AppearancesCore) -> Router {
    Router::new()
        .route("/appearances", routing::get(places_of))
        .with_state(appearances)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppearanceChanges(pub String);

pub struct AppearancesTopic {
    appearances: AppearancesCore,
    broadcaster: Broadcaster<AppearanceChanges>,
}

impl AppearancesTopic {
    pub fn new(appearances: AppearancesCore, broadcaster: Broadcaster<AppearanceChanges>) -> Self {
        Self {
            appearances,
            broadcaster,
        }
    }
}

#[async_trait]
impl Topic for AppearancesTopic {
    fn named(&self) -> &'static str {
        "appearances"
    }

    async fn subscribe(&self, agent: &Agent, about: &About) -> Result<Subscribed, Refused> {
        let Some(idea) = about.get("idea") else {
            return Err(Refused::NotThere);
        };

        match self.appearances.may_follow(idea, agent).await {
            Ok(true) => Ok(Subscribed::hearing(
                self.broadcaster.hear(AppearanceChanges(idea.clone())),
            )
            .sifted(Arc::new(ReadableBy {
                agent: agent.clone(),
                appearances: self.appearances.clone(),
            }))),
            Ok(false) => Err(Refused::NotThere),
            Err(_) => Err(Refused::Failed),
        }
    }
}

pub struct TellingAppearances {
    kept: Arc<dyn AppearanceCatalog>,
    teller: Teller<AppearanceChanges>,
}

impl TellingAppearances {
    pub fn around(
        kept: Arc<dyn AppearanceCatalog>,
        broadcaster: Broadcaster<AppearanceChanges>,
    ) -> Self {
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
        self.teller.tell(
            AppearanceChanges(idea.as_str().to_owned()),
            idea.as_str(),
            event,
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
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        self.kept
            .forget_tombstones_laid_before(before, at_most)
            .await
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
    State(appearances): State<AppearancesCore>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<OfIdea>,
) -> Result<Json<Vec<PlaceDTO>>, Problem> {
    let places =
        appearances
            .places_of(&asked.idea, &agent)
            .await
            .map_err(|refusal| match refusal {
                AppearancesError::IdeaNotThere => Problem::not_found(serving::NOT_FOUND, refusal),
                beyond_reach => Problem::unexpected(beyond_reach),
            })?;

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
