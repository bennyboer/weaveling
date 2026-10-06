use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;

use crate::appearance::{Place, Subject};

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("the catalog of appearances could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait AppearanceCatalog: Send + Sync {
    async fn remember(&self, subject: &Subject, place: &Place) -> Result<(), CatalogError>;

    async fn forget(&self, subject: &Subject, place: &Place) -> Result<(), CatalogError>;

    async fn forget_subject(&self, subject: &Subject) -> Result<(), CatalogError>;

    async fn forget_place(&self, place: &Place) -> Result<(), CatalogError>;

    async fn places_of(&self, subject: &Subject) -> Result<Vec<Place>, CatalogError>;
}
