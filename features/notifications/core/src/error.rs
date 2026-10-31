use eventsourcing::ServiceError;
use ids::InvalidId;
use thiserror::Error;

use crate::catalog::CatalogError;
use crate::notice::NoticeError;

#[derive(Debug, Error)]
pub enum NotificationsError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    Events(#[from] ServiceError<NoticeError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}
