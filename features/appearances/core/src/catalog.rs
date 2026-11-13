use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;
use transacting::Transaction;

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
        transaction: &mut Transaction,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError>;

    async fn forget(
        &self,
        transaction: &mut Transaction,
        subject: &Subject,
        place: &Place,
        version: u64,
    ) -> Result<(), CatalogError>;

    async fn forget_subject(
        &self,
        transaction: &mut Transaction,
        subject: &Subject,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError>;

    async fn forget_place(
        &self,
        transaction: &mut Transaction,
        place: &Place,
        at: OffsetDateTime,
    ) -> Result<Vec<Subject>, CatalogError>;

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError>;

    /// Returns the places a subject appears in that belong to its own project.
    /// Returns none while the subject's project is unknown.
    async fn places_of(&self, subject: &Subject) -> Result<Vec<Place>, CatalogError>;

    async fn remember_project(
        &self,
        transaction: &mut Transaction,
        subject: &Subject,
        project: &ProjectLink,
    ) -> Result<(), CatalogError>;

    async fn project_of(&self, subject: &Subject) -> Result<Option<ProjectLink>, CatalogError>;

    /// Forgets every place of a deleted project, the same way `forget_place` forgets one.
    /// Returns the subjects that appeared in those places.
    async fn forget_places_of_project(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<Vec<Subject>, CatalogError>;

    /// Stores which project a place belongs to, until the place is forgotten.
    /// Does nothing for a place that is already forgotten.
    async fn remember_place_project(
        &self,
        transaction: &mut Transaction,
        place: &Place,
        project: &ProjectLink,
    ) -> Result<(), CatalogError>;
}
