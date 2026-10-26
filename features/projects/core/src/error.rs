use authorizing::AuthorizerError;
use eventsourcing::ServiceError;
use ids::InvalidId;
use thiserror::Error;

use crate::catalog::CatalogError;
use crate::details::{InvalidBookLanguage, InvalidProjectDescription, InvalidQuotationMarks};
use crate::name::InvalidProjectName;
use crate::project::ProjectError;

#[derive(Debug, Error)]
pub enum ProjectsError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    InvalidName(#[from] InvalidProjectName),
    #[error(transparent)]
    InvalidLanguage(#[from] InvalidBookLanguage),
    #[error(transparent)]
    InvalidQuotation(#[from] InvalidQuotationMarks),
    #[error(transparent)]
    InvalidDescription(InvalidProjectDescription),
    #[error(transparent)]
    Events(#[from] ServiceError<ProjectError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Authorizer(#[from] AuthorizerError),
    #[error("only an owner of the project may do this")]
    NotAnOwner,
}
