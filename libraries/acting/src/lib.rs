use std::future;
use std::sync::Arc;

use async_trait::async_trait;
use eventsourcing::Agent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Acting {
    pub agent: Agent,
    pub operator: bool,
}

#[async_trait]
pub trait Ending: Send + Sync {
    async fn reached(&self);
}

#[derive(Clone)]
pub struct SessionEnd(Option<Arc<dyn Ending>>);

impl SessionEnd {
    pub fn of(ending: Arc<dyn Ending>) -> Self {
        Self(Some(ending))
    }

    pub fn never() -> Self {
        Self(None)
    }

    pub async fn reached(&self) {
        match &self.0 {
            Some(ending) => ending.reached().await,
            None => future::pending().await,
        }
    }
}
