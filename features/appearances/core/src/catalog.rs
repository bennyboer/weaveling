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

    /// The places a subject appears in that are of its own project, none while its project is unknown.
    async fn places_of(&self, subject: &Subject) -> Result<Vec<Place>, CatalogError>;

    async fn remember_project(
        &self,
        transaction: &mut Transaction,
        subject: &Subject,
        project: &ProjectLink,
    ) -> Result<(), CatalogError>;

    async fn project_of(&self, subject: &Subject) -> Result<Option<ProjectLink>, CatalogError>;

    /// Forgets every place of a deleted project as `forget_place` forgets one, telling which subjects appeared in them.
    async fn forget_places_of_project(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<Vec<Subject>, CatalogError>;

    /// Keeps the project a place is of until the place is forgotten; a place forgotten already keeps none.
    async fn remember_place_project(
        &self,
        transaction: &mut Transaction,
        place: &Place,
        project: &ProjectLink,
    ) -> Result<(), CatalogError>;
}
