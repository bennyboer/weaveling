use authorizing::AuthorizerError;
use eventsourcing::ServiceError;
use ids::InvalidId;
use registry::RegistryError;
use thiserror::Error;

use crate::catalog::CatalogError;
use crate::outline::OutlineError;

#[derive(Debug, Error)]
pub enum OutlinesError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    Events(#[from] ServiceError<OutlineError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Registry(#[from] RegistryError),
    #[error(transparent)]
    Authorizer(#[from] AuthorizerError),
    #[error("there is no such project")]
    ProjectNotThere,
    #[error("only an author of the project may do this")]
    NotAnAuthor,
}
