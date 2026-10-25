use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;

use crate::appearance::{Place, Subject};
use crate::link::ProjectLink;

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("the catalog of appearances could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait AppearanceCatalog: Send + Sync {
    async fn remember(
        &self,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError>;

    async fn forget(
        &self,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError>;

    async fn forget_subject(
        &self,
        subject: &Subject,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn forget_place(
        &self,
        place: &Place,
        at: OffsetDateTime,
    ) -> Result<Vec<Subject>, CatalogError>;

    async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;

    async fn places_of(&self, subject: &Subject) -> Result<Vec<Place>, CatalogError>;

    async fn remember_project(
        &self,
        subject: &Subject,
        project: &ProjectLink,
    ) -> Result<(), CatalogError>;

    async fn project_of(&self, subject: &Subject) -> Result<Option<ProjectLink>, CatalogError>;
}
