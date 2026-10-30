use eventsourcing::ServiceError;
use ids::InvalidId;
use registry::RegistryError;
use thiserror::Error;

use crate::notice::NoticeError;

#[derive(Debug, Error)]
pub enum NotificationsError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    Events(#[from] ServiceError<NoticeError>),
    #[error(transparent)]
    Registry(#[from] RegistryError),
}
