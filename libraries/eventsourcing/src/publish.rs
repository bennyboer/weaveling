use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use crate::event::Recorded;

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("what happened could not be published")]
    NotHandled(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait EventPublisher<E>: Send + Sync {
    async fn publish(&self, happened: &Recorded<E>) -> Result<(), PublishError>;
}

pub struct NoopEventPublisher;

impl PublishError {
    pub fn because(reason: impl Error + Send + Sync + 'static) -> Self {
        Self::NotHandled(Box::new(reason))
    }
}

impl NoopEventPublisher {
    pub fn shared<E>() -> Arc<dyn EventPublisher<E>>
    where
        E: Send + Sync + 'static,
    {
        Arc::new(Self)
    }
}

#[async_trait]
impl<E> EventPublisher<E> for NoopEventPublisher
where
    E: Send + Sync,
{
    async fn publish(&self, _happened: &Recorded<E>) -> Result<(), PublishError> {
        Ok(())
    }
}
