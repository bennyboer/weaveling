use eventsourcing::ServiceError;
use thiserror::Error;

use crate::catalog::CatalogError;
use crate::members::MembersError;

#[derive(Debug, Error)]
pub enum MembersCoreError {
    #[error(transparent)]
    Events(#[from] ServiceError<MembersError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}
