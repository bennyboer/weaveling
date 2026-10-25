use authorizing::AuthorizerError;
use eventsourcing::ServiceError;
use ids::InvalidId;
use thiserror::Error;

use crate::catalog::CatalogError;
use crate::idea::IdeaError;
use crate::title::InvalidIdeaTitle;

#[derive(Debug, Error)]
pub enum IdeasError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    InvalidTitle(#[from] InvalidIdeaTitle),
    #[error(transparent)]
    Events(#[from] ServiceError<IdeaError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Authorizer(#[from] AuthorizerError),
    #[error("there is no such project")]
    ProjectNotThere,
    #[error("only an author of the project may do this")]
    NotAnAuthor,
}
