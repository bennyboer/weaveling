use std::error::Error;

use async_trait::async_trait;
use eventsourcing::Recorded;
use thiserror::Error;

use crate::piece::PieceEvent;

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("what happened to a piece could not be published")]
    NotHandled(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait PieceEventPublisher: Send + Sync {
    async fn publish(&self, happened: &Recorded<PieceEvent>) -> Result<(), PublishError>;
}

impl PublishError {
    pub fn because(reason: impl Error + Send + Sync + 'static) -> Self {
        Self::NotHandled(Box::new(reason))
    }
}
